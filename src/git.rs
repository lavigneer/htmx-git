use itertools::Itertools;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use git2::{BranchType, Repository};

pub struct GitWrapper {
    repo: Repository,
}

#[derive(Eq, PartialEq)]
pub struct Commit {
    pub id: String,
    pub message: String,
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

    pub fn list_local_branches(&self) -> Vec<String> {
        self.repo
            .branches(Some(BranchType::Local))
            .unwrap()
            .into_iter()
            // TODO: Fix all this unwrapping
            .map(|b| b.unwrap().0.name().unwrap().unwrap().to_owned())
            .collect::<Vec<String>>()
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
        let remote = self.repo.find_remote(remote)?;
        Ok(remote
            .fetch_refspecs()?
            .into_iter()
            .flat_map(|s| s.and_then(|b| Some(b.to_string())))
            .collect())
    }

    pub fn checkout_local_branch(&self, branch: &str) -> Result<(), git2::Error> {
        let branch_ref = &format!("refs/heads/{}", branch);
        let obj = self.repo.revparse_single(branch_ref).unwrap();
        let _ = self.repo.checkout_tree(&obj, None)?;
        let _ = self.repo.set_head(branch_ref)?;
        Ok(())
    }

    pub fn list_commits<'a>(
        &'a self,
        branch: &str,
        filter: Option<&'a str>,
    ) -> impl Iterator<Item = Commit> + 'a {
        let branch_ref = &format!("refs/heads/{}", branch);
        let obj = self.repo.revparse_single(branch_ref).unwrap();
        let mut revwalk = self.repo.revwalk().unwrap();
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();
        revwalk.push(obj.id()).unwrap();

        let matcher = SkimMatcherV2::default();
        revwalk
            .into_iter()
            .filter_map(move |id| match (filter, id) {
                (_, Ok(id)) => match (filter, self.repo.find_commit(id)) {
                    (Some(filter), Ok(commit)) => {
                        let message = commit.message().unwrap_or("UNKNOWN").to_owned();
                        let score = matcher.fuzzy_match(&message, &filter);
                        return score.and_then(|score| {
                            Some(Commit {
                                id: id.to_string(),
                                message,
                                sort_score: score,
                            })
                        });
                    }
                    (None, Ok(commit)) => Some(Commit {
                        id: id.to_string(),
                        message: commit.message().unwrap_or("UNKNOWN").to_owned(),
                        sort_score: 0,
                    }),
                    (Some(_), Err(_err)) => None,
                    (None, Err(_err)) => Some(Commit {
                        id: id.to_string(),
                        message: "Error Finding Commit".to_owned(),
                        sort_score: 0,
                    }),
                },
                (Some(_), Err(_err)) => None,
                (None, Err(_err)) => Some(Commit {
                    id: "".to_owned(),
                    message: "Error Finding Commit".to_owned(),
                    sort_score: 0,
                }),
            }).sorted()
    }
}
