use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use git2::{BranchType, Repository};

pub struct GitWrapper {
    repo: Repository,
}

pub struct Commit {
    pub id: String,
    pub message: String,
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

    pub fn list_branches(&self) -> Vec<String> {
        self.repo
            .branches(Some(BranchType::Local))
            .unwrap()
            .into_iter()
            // TODO: Fix all this unwrapping
            .map(|b| b.unwrap().0.name().unwrap().unwrap().to_owned())
            .collect::<Vec<String>>()
    }

    pub fn checkout_local_branch(&self, branch: &str) -> Result<(), git2::Error> {
        let branch_ref = &format!("refs/heads/{}", branch);
        let obj = self.repo.revparse_single(branch_ref).unwrap();
        let _ = self.repo.checkout_tree(&obj, None)?;
        let _ = self.repo.set_head(branch_ref)?;
        Ok(())
    }

    pub fn list_commits(&self, branch: &str, filter: Option<&str>) -> impl Iterator<Item = Commit> {
        let branch_ref = &format!("refs/heads/{}", branch);
        let obj = self.repo.revparse_single(branch_ref).unwrap();
        let mut revwalk = self.repo.revwalk().unwrap();
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();
        revwalk.push(obj.id()).unwrap();

        let matcher = SkimMatcherV2::default();
        let mut commits: Vec<(i64, Commit)> = revwalk
            .into_iter()
            .filter_map(|id| match (filter, id) {
                (_, Ok(id)) => match (filter, self.repo.find_commit(id)) {
                    (Some(filter), Ok(commit)) => {
                        let message = commit.message().unwrap_or("UNKNOWN").to_owned();
                        let score = matcher.fuzzy_match(&message, filter);
                        return score.and_then(|score| {
                            Some((
                                score,
                                Commit {
                                    id: id.to_string(),
                                    message,
                                },
                            ))
                        });
                    }
                    (None, Ok(commit)) => Some((
                        0,
                        Commit {
                            id: id.to_string(),
                            message: commit.message().unwrap_or("UNKNOWN").to_owned(),
                        },
                    )),
                    (Some(_), Err(_err)) => None,
                    (None, Err(_err)) => Some((
                        0,
                        Commit {
                            id: id.to_string(),
                            message: "Error Finding Commit".to_owned(),
                        },
                    )),
                },
                (Some(_), Err(_err)) => None,
                (None, Err(_err)) => Some((
                    0,
                    Commit {
                        id: "".to_owned(),
                        message: "Error Finding Commit".to_owned(),
                    },
                )),
            })
            .collect();
        commits.sort_by(|a, b| b.0.cmp(&a.0));
        commits.into_iter().map(|(_, c)| c)
    }
}
