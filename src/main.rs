use axum::error_handling::HandleErrorLayer;
use axum::http::StatusCode;
use axum::Router;
use axum::routing::get;
use color_eyre::eyre;
use maud::{DOCTYPE, html, Markup, PreEscaped};
use sqlx::PgPool;
use tower::{BoxError, ServiceBuilder};
use tower_sessions::cookie::time;
use tower_sessions::{CachingSessionStore, Expiry, MokaStore, PostgresStore, SessionManagerLayer};
use tracing::error;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    if std::path::Path::new(".env").exists() {
        dotenvy::dotenv()?;
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let db = PgPool::connect(&std::env::var("DATABASE_URL")?).await?;

    #[cfg(not(debug_assertions))]
    sqlx::migrate!().run(&db).await?;

    let state = AppState { db };

    // on larger deployments this should be replaced with redis where redis evict old sessions automatically
    let moka_store = MokaStore::new(Some(1024));
    let postgres_store = PostgresStore::new(state.db.clone());
    let store = CachingSessionStore::new(moka_store, postgres_store);

    let session_service = ServiceBuilder::new()
        .layer(
            SessionManagerLayer::new(store)
                // Setting this to false as will likely operate behind a reverse proxy
                // Also for development
                // FIXME: Review before deployment
                .with_secure(false)
                .with_expiry(Expiry::OnInactivity(time::Duration::days(7))),
        );

    let app = routes().layer(session_service).with_state(state);

    let bind_addr = std::env::var("BIND_ADDR")?;
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/test", get(test))
}

async fn index() -> Markup {
    html!(
        (DOCTYPE)
        html {
            head {
                title { "Hello World" }
                script src="https://unpkg.com/htmx.org@1.9" defer;
            }
            body {
                h1 { "Hello World" }
                button hx-get="/test" hx-swap="outerHTML" { "Click me" }
            }
        }
    )
}

async fn test() -> Markup {
    html!(
        "Hi ^^"
    )
}
