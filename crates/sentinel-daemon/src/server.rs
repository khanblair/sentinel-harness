//! Binds the daemon to the tailnet address and serves the API, the WebSocket, and the UI.

use std::net::SocketAddr;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use tokio::net::TcpListener;
use tower_http::limit::RequestBodyLimitLayer;

use crate::assets;
use crate::auth;
use crate::engine::Engine;
use crate::routes;
use crate::tailscale::{self, BindError, BindMode};
use crate::ws;

/// The largest request body the API takes. Bigger bodies get `413`.
pub const BODY_LIMIT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerOptions {
    pub port: u16,
    pub bind: BindMode,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("the daemon cannot bind here: {0}")]
    Bind(#[from] BindError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// A listener that has its address and is ready to serve.
pub struct BoundServer {
    pub addr: SocketAddr,
    listener: TcpListener,
    router: Router,
}

impl BoundServer {
    /// Serves until the returned future is dropped or fails.
    pub async fn serve(self) -> std::io::Result<()> {
        axum::serve(self.listener, self.router).await
    }
}

/// Picks the tailnet address from Tailscale and binds to it. A wildcard address is refused.
pub async fn bind(engine: Engine, options: &ServerOptions) -> Result<BoundServer, ServerError> {
    let status = engine.tailscale_status().await;
    let ip = tailscale::bind_address(&status, options.bind)?;
    let listener = TcpListener::bind(SocketAddr::new(ip, options.port)).await?;
    let addr = listener.local_addr()?;
    Ok(BoundServer {
        addr,
        listener,
        router: router(engine),
    })
}

/// The whole app. Only the routes that are not public pass through the device check.
pub fn router(engine: Engine) -> Router {
    let protected = routes::protected_routes().route_layer(middleware::from_fn_with_state(
        engine.clone(),
        auth::require_device,
    ));
    Router::new()
        .merge(routes::public_routes())
        .merge(protected)
        .route("/ws", get(ws::upgrade))
        .fallback(assets::serve)
        .layer(middleware::from_fn(auth::check_origin))
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
        .with_state(engine)
}
