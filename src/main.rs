use std::sync::Arc;
use std::{net::SocketAddr, sync::Mutex};

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use git2::Repository;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    repo: Repository,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    branches: Vec<String>,
}

async fn index(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let repo = &state.lock().unwrap().repo;
    let branches = repo
        .branches(None)
        .unwrap()
        .into_iter()
        .map(|b| b.unwrap().0.name().unwrap().unwrap().to_owned())
        .collect::<Vec<String>>();
    let template = IndexTemplate { branches };
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

    let repo = Repository::open("/home/elavigne/workspace/htmx-git-client/test-repo").unwrap();
    let shared_state = Arc::new(Mutex::new(AppState { repo }));

    let assets_path = std::env::current_dir().unwrap();
    let app = Router::new()
        .route("/", get(index))
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
