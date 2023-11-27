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
use htmx_git_client::git::{Commit, GitWrapper};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    repo: GitWrapper,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    current_branch: String,
    branches: Vec<String>,
}

async fn index(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let branches = repo.list_branches();
    let current_branch = repo.get_current_branch().unwrap();
    let template = IndexTemplate {
        current_branch,
        branches,
    };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "log.html")]
struct LogTemplate {
    current_branch: String,
    branches: Vec<String>,
}

async fn log(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let current_branch = repo.get_current_branch().unwrap();
    let branches = repo.list_branches();
    let template = LogTemplate {
        current_branch,
        branches,
    };
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "log_list.html")]
struct LogListTemplate {
    commits: Vec<Commit>,
    current_page: usize,
    current_filter: String,
    current_branch: String,
}

async fn log_list(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(branch): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let filter = params.get("filter").map(|f| f.as_str());
    let commits = repo.list_commits(&branch, filter);
    let page: usize = match params.get("page") {
        Some(s) => s.parse().unwrap_or(0),
        None => 0,
    };
    let commits = commits.skip(page * 100).take(100).collect::<Vec<Commit>>();
    let template = LogListTemplate {
        commits,
        current_branch: branch,
        current_page: page,
        current_filter: match filter {
            None => "".to_string(),
            Some(filter) => filter.to_string(),
        },
    };
    HtmlTemplate(template)
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
    repo.checkout_local_branch(&branch).unwrap();
    let branches = repo.list_branches();
    let current_branch = repo.get_current_branch().unwrap();
    let template = BranchListTemplate {
        current_branch,
        branches,
        out_of_band: true,
    };
    HtmlTemplate(template)
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
