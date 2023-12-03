use std::fmt::Display;

use anyhow::Result;
use chrono::{FixedOffset, NaiveDateTime};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use git2::{BranchType, DiffFormat, Repository, Time};
use itertools::Itertools;

pub struct GitWrapper {
    repo: Repository,
}

#[derive(Eq, PartialEq)]
pub struct CommitDate(Time);

impl Display for CommitDate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let offset = FixedOffset::east_opt(self.0.offset_minutes() * 60).ok_or(std::fmt::Error)?;
        let date_time =
            NaiveDateTime::from_timestamp_opt(self.0.seconds(), 0).ok_or(std::fmt::Error)?;
        let date_time = date_time
            .and_local_timezone(offset)
            .single()
            .ok_or(std::fmt::Error)?;
        write!(f, "{}", date_time.to_rfc2822())
    }
}

#[derive(Eq, PartialEq)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: String,
    pub date: CommitDate,
    sort_score: i64,
}

impl PartialOrd for Commit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Commit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_score.cmp(&other.sort_score)
    }
}

pub struct DiffLineData {
    pub content: String,
    pub operation: DiffLineOperation,
    pub old_line_number: Option<u32>,
    pub new_line_number: Option<u32>,
}

pub enum DiffLineOperation {
    Context,
    Addition,
    Deletion,
    ContextEOF,
    AddEOF,
    RemoveEOF,
    FileHeader,
    HunkHeader,
    Binary,
}

impl GitWrapper {
    pub fn new(repo: &str) -> Result<Self, git2::Error> {
        let repo = Repository::open(repo)?;
        Ok(Self { repo })
    }

    pub fn get_current_branch(&self) -> Result<String, git2::Error> {
        Ok(self
            .repo
            .head()?
            .shorthand()
            .ok_or(git2::Error::from_str("Ihvalid utf-8 branch name"))?
            .to_owned())
    }

    pub fn list_local_branches(&self) -> Result<Vec<String>> {
        Ok(self
            .repo
            .branches(Some(BranchType::Local))?
            .into_iter()
            .filter_map(|b| match b.ok()?.0.name() {
                Ok(Some(name)) => Some(name.to_owned()),
                _ => None,
            })
            .collect::<Vec<String>>())
    }

    pub fn list_remotes(&self) -> Result<Vec<String>, git2::Error> {
        Ok(self
            .repo
            .remotes()?
            .into_iter()
            .flat_map(|r| r.and_then(|r| Some(r.to_string())))
            .collect())
    }

    pub fn list_remote_branches(&self, remote: &str) -> Result<Vec<String>, git2::Error> {
        let mut remote = self.repo.find_remote(remote)?;
        remote.connect(git2::Direction::Fetch)?;
        Ok(remote
            .list()?
            .into_iter()
            .map(|head| head.name())
            .filter(|head_name| head_name.starts_with("refs/heads/"))
            .map(|head_name| head_name.replace("refs/heads/", ""))
            .collect())
    }

    pub fn commit_diff(&self, sha: &str) -> Result<Vec<DiffLineData>, git2::Error> {
        let commit = self.repo.find_commit(git2::Oid::from_str(sha)?)?;
        let commit_tree = commit.tree()?;
        let commit_parent = commit
            .parents()
            .next()
            .ok_or(git2::Error::from_str("Could not find parent commit."))?;
        let commit_parent_tree = commit_parent.tree()?;
        let diff =
            self.repo
                .diff_tree_to_tree(Some(&commit_parent_tree), Some(&commit_tree), None)?;

        let mut result = Vec::new();
        diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
            let content = std::str::from_utf8(line.content())
                .unwrap()
                .trim_end()
                .to_string();
            result.push(DiffLineData {
                content,
                operation: match line.origin() {
                    '+' => DiffLineOperation::Addition,
                    '-' => DiffLineOperation::Deletion,
                    '=' => DiffLineOperation::Context,
                    '>' => DiffLineOperation::AddEOF,
                    '<' => DiffLineOperation::RemoveEOF,
                    'F' => DiffLineOperation::FileHeader,
                    'H' => DiffLineOperation::HunkHeader,
                    'B' => DiffLineOperation::Binary,
                    _ => DiffLineOperation::Context,
                },
                old_line_number: line.old_lineno(),
                new_line_number: line.new_lineno(),
            });
            true
        })?;
        Ok(result)
    }

    pub fn checkout_local_branch(&self, branch: &str) -> Result<(), git2::Error> {
        let branch_ref = &format!("refs/heads/{}", branch);
        let obj = self.repo.revparse_single(branch_ref)?;
        let _ = self.repo.checkout_tree(&obj, None)?;
        let _ = self.repo.set_head(branch_ref)?;
        Ok(())
    }

    pub fn list_commits<'a>(
        &'a self,
        reference: &str,
        filter: Option<&'a str>,
    ) -> Result<impl Iterator<Item = Commit> + 'a> {
        let obj = self.repo.revparse_single(reference)?;
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;
        revwalk.push(obj.id())?;

        let matcher = SkimMatcherV2::default();
        let result = revwalk.filter_map(move |id| match id {
            Ok(id) => match (filter, self.repo.find_commit(id)) {
                (Some(filter), Ok(commit)) => {
                    let message = commit.message().unwrap_or("UNKNOWN").to_owned();
                    let score = matcher.fuzzy_match(&message, &filter);
                    return score.and_then(|score| {
                        Some(Commit {
                            id: id.to_string(),
                            message,
                            author: commit.author().to_string(),
                            date: CommitDate(commit.time()),
                            sort_score: score,
                        })
                    });
                }
                (None, Ok(commit)) => Some(Commit {
                    id: id.to_string(),
                    message: commit.message().unwrap_or("UNKNOWN").to_owned(),
                    author: commit.author().to_string(),
                    date: CommitDate(commit.time()),
                    sort_score: 0,
                }),
                _ => None,
            },
            _ => None,
        });
        match filter {
            Some(_) => Ok(itertools::Either::Right(result.sorted())),
            None => Ok(itertools::Either::Left(result)),
        }
    }
}
