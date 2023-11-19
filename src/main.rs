use std::sync::Arc;
use std::{net::SocketAddr, sync::Mutex};

use askama::Template;
use axum::extract::Path;
use axum::routing::patch;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use git2::{Repository, BranchType};
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
        out_of_band
    }
}

#[derive(Template)]
#[template(path = "branch_list.html")]
struct BranchListTemplate {
    current_branch: String,
    branches: Vec<String>,
    out_of_band: bool
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
