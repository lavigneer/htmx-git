use std::collections::HashMap;
use std::sync::Arc;
use std::{net::SocketAddr, sync::Mutex};

use askama::Template;
use axum::extract::{Path, Query};
use axum::response::Redirect;
use axum::routing::patch;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use git2::DiffFormat;
use htmx_git_client::git::{Commit, GitWrapper, DiffLine};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    repo: GitWrapper,
}

async fn index(State(state): State<Arc<Mutex<AppState>>>) -> Result<impl IntoResponse, AppError> {
    let repo = &state
        .lock()
        .map_err(|_| anyhow::anyhow!("Could not get reference to repo"))?
        .repo;
    let current_branch = repo.get_current_branch()?;
    Ok(Redirect::to(&format!("/log/refs/heads/{}", current_branch)))
}

#[derive(Template)]
#[template(path = "log.html")]
struct LogTemplate {
    current_branch: String,
    branches: Vec<String>,
    commits: Vec<Commit>,
    current_page: usize,
    current_filter: String,
    remotes: Vec<String>,
}

async fn log(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(reference): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let repo = &state
        .lock()
        .map_err(|_| anyhow::anyhow!("Could not get reference to repo"))?
        .repo;
    let current_branch = repo.get_current_branch()?;

    let filter = params.get("filter").and_then(|f| match f.is_empty() {
        true => None,
        false => Some(f.as_str()),
    });
    let commits = repo.list_commits(&reference, filter);
    let page: usize = match params.get("page") {
        Some(s) => s.parse().unwrap_or(0),
        None => 0,
    };
    let commits = commits?.skip(page * 100).take(100).collect::<Vec<Commit>>();

    let remotes = repo.list_remotes()?;
    let branches = repo.list_local_branches()?;
    let template = LogTemplate {
        commits,
        current_branch,
        branches,
        remotes,
        current_page: page,
        current_filter: match filter {
            None => "".to_string(),
            Some(filter) => filter.to_string(),
        },
    };
    Ok(HtmlTemplate(template))
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
) -> Result<impl IntoResponse, AppError> {
    let repo = &state
        .lock()
        .map_err(|_| anyhow::anyhow!("Could not get reference to repo"))?
        .repo;
    repo.checkout_local_branch(&branch)?;
    let branches = repo.list_local_branches()?;
    let current_branch = repo.get_current_branch()?;
    let template = BranchListTemplate {
        current_branch,
        branches,
        out_of_band: true,
    };
    Ok(HtmlTemplate(template))
}

#[derive(Template)]
#[template(path = "remote_branch_list.html")]
struct RemoteBranchListTemplate {
    remote: String,
    branches: Vec<String>,
    open: bool,
}
async fn remote_branch_list(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(remote): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let open = params
        .get("open")
        .unwrap_or(&"false".to_string())
        .parse::<bool>()
        .unwrap_or(true);
    let repo = &state
        .lock()
        .map_err(|_| anyhow::anyhow!("Could not get reference to repo"))?
        .repo;
    let branches = match open {
        true => repo.list_remote_branches(&remote).unwrap(),
        false => vec![],
    };
    let template = RemoteBranchListTemplate {
        branches,
        remote,
        open,
    };
    Ok(HtmlTemplate(template))
}

#[derive(Template)]
#[template(path = "view_commit.html")]
struct ViewCommitTemplate {
    diffs: Vec<DiffLine>,
}

async fn view_commit(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(sha): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let repo = &state
        .lock()
        .map_err(|_| anyhow::anyhow!("Could not get reference to repo"))?
        .repo;
    let diffs = repo.commit_diff(&sha)?;
    let template = ViewCommitTemplate { diffs };
    Ok(HtmlTemplate(template))
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

    let repo = GitWrapper::new("/home/elavigne/workspace/htmx-git-client/test-repo").unwrap();
    let shared_state = Arc::new(Mutex::new(AppState { repo }));

    let assets_path = std::env::current_dir().unwrap();
    let app = Router::new()
        .route("/", get(index))
        .route("/log/*reference", get(log))
        .route("/remote/branches/*remote", get(remote_branch_list))
        .route("/checkout/*branch", patch(checkout_branch))
        .route("/commit/*sha", get(view_commit))
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

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}
