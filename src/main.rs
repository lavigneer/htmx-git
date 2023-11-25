use std::collections::HashMap;
use std::sync::Arc;
use std::{net::SocketAddr, sync::Mutex};

use askama::Template;
use axum::extract::{Path, Query};
use axum::routing::patch;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use git2::{BranchType, Repository};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    repo: Repository,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    branch_list: BranchListTemplate,
}

async fn index(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let template = IndexTemplate {
        branch_list: build_branch_list_template(repo, false),
    };
    HtmlTemplate(template)
}

struct CommitRow {
    id: String,
    message: String,
}

#[derive(Template)]
#[template(path = "log.html")]
struct LogTemplate {
    branch_list: BranchListTemplate,
}

#[derive(Template)]
#[template(path = "log_list.html")]
struct LogListTemplate<'a> {
    commits: Vec<&'a CommitRow>,
}

async fn log_list(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(branch): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let branch_ref = &format!("refs/heads/{}", branch);
    let obj = repo.revparse_single(branch_ref).unwrap();
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.set_sorting(git2::Sort::TOPOLOGICAL).unwrap();
    revwalk.push(obj.id()).unwrap();
    let filter = params.get("filter");
    let page: usize = match params.get("page") {
        Some(s) => s.parse().unwrap_or(0),
        None => 0,
    };
    let matcher = SkimMatcherV2::default();
    let mut commits: Vec<(i64, CommitRow)> = revwalk
        .into_iter()
        .filter_map(|id| match (filter, id) {
            (_, Ok(id)) => match (filter, repo.find_commit(id)) {
                (Some(filter), Ok(commit)) => {
                    let message = commit.message().unwrap_or("UNKNOWN").to_owned();
                    let score = matcher.fuzzy_match(&message, filter);
                    return score.and_then(|score| {
                        Some((
                            score,
                            CommitRow {
                                id: id.to_string(),
                                message,
                            },
                        ))
                    });
                }
                (None, Ok(commit)) => Some((
                    0,
                    CommitRow {
                        id: id.to_string(),
                        message: commit.message().unwrap_or("UNKNOWN").to_owned(),
                    },
                )),
                (Some(_), Err(_err)) => None,
                (None, Err(_err)) => Some((
                    0,
                    CommitRow {
                        id: id.to_string(),
                        message: "Error Finding Commit".to_owned(),
                    },
                )),
            },
            (Some(_), Err(_err)) => None,
            (None, Err(_err)) => Some((
                0,
                CommitRow {
                    id: "".to_owned(),
                    message: "Error Finding Commit".to_owned(),
                },
            )),
        })
        .collect();
    commits.sort_by(|a, b| b.0.cmp(&a.0));
    let commits = commits[page * 100..(page + 1) * 100]
        .iter()
        .map(|(_, c)| c)
        .collect::<Vec<&CommitRow>>();
    let template = LogListTemplate { commits };
    return match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to render template. Error: {err}"),
        )
            .into_response(),
    };
}

async fn log(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let template = LogTemplate {
        branch_list: build_branch_list_template(repo, false),
    };
    HtmlTemplate(template)
}

fn build_branch_list_template(repo: &Repository, out_of_band: bool) -> BranchListTemplate {
    let branches = repo
        .branches(Some(BranchType::Local))
        .unwrap()
        .into_iter()
        // TODO: Fix all this unwrapping
        .map(|b| b.unwrap().0.name().unwrap().unwrap().to_owned())
        .collect::<Vec<String>>();
    let current_branch = repo.head().unwrap().shorthand().unwrap().to_owned();
    BranchListTemplate {
        current_branch,
        branches,
        out_of_band,
    }
}

#[derive(Template)]
#[template(path = "branch_list.html")]
struct BranchListTemplate {
    current_branch: String,
    branches: Vec<String>,
    out_of_band: bool,
}
async fn checkout_branch(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(branch): Path<String>,
) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let branch_ref = &format!("refs/heads/{}", branch);
    let obj = repo.revparse_single(branch_ref).unwrap();
    let _ = repo.checkout_tree(&obj, None);
    let _ = repo.set_head(branch_ref);
    HtmlTemplate(build_branch_list_template(repo, true))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "htmx_git_client=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let repo = Repository::open("/home/elavigne/workspace/htmx-git-client/test-repo").unwrap();
    let shared_state = Arc::new(Mutex::new(AppState { repo }));

    let assets_path = std::env::current_dir().unwrap();
    let app = Router::new()
        .route("/", get(index))
        .route("/log/*branch", get(log))
        .route("/log/list/*branch", get(log_list))
        .route("/checkout/*branch", patch(checkout_branch))
        .with_state(shared_state)
        .nest_service(
            "/assets",
            ServeDir::new(format!("{}/assets", assets_path.to_str().unwrap())),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

struct HtmlTemplate<T>(T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}
