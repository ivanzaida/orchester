//! The HTTP side of the server: streamable HTTP at `/mcp` on `127.0.0.1`, on a port the OS picks, on
//! the application's runtime. Every request must carry the bearer token of the discovery file; a
//! request without it is refused before the MCP layer reads it.

use std::{
  io,
  net::{Ipv4Addr, SocketAddr, TcpListener},
  num::NonZeroU16,
  sync::Arc,
};

use axum::{
  Router,
  extract::{Request, State},
  http::{StatusCode, header::AUTHORIZATION},
  middleware::{self, Next},
  response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
  StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::{runtime::Handle, sync::oneshot};

use super::handler::Handler;

/// The only address the server listens on.
const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;
const PATH: &str = "/mcp";

/// `127.0.0.1:<port>`.
pub fn address(port: NonZeroU16) -> String {
  SocketAddr::from((LOOPBACK, port.get())).to_string()
}

/// `http://127.0.0.1:<port>/mcp`, the endpoint clients connect to.
pub fn endpoint(port: NonZeroU16) -> String {
  format!("http://{}{PATH}", address(port))
}

/// The bearer token clients present. It is a secret: it shows in no `Debug` output and has no
/// `Display`.
#[derive(Clone)]
pub struct BearerToken(Arc<str>);

impl std::fmt::Debug for BearerToken {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("BearerToken(<redacted>)")
  }
}

impl BearerToken {
  /// A new token from the OS random source: two random UUIDs, 244 random bits.
  pub fn generate() -> Self {
    Self(format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple()).into())
  }

  pub fn expose(&self) -> &str {
    &self.0
  }

  /// Whether `presented` is this token, compared in constant time so the comparison does not tell how
  /// much of it matched.
  fn admits(&self, presented: &str) -> bool {
    let (expected, presented) = (self.0.as_bytes(), presented.as_bytes());
    expected.len() == presented.len()
      && expected
        .iter()
        .zip(presented)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
  }
}

/// The server, listening; dropping it stops it.
pub struct Listening {
  port: NonZeroU16,
  stop: Option<oneshot::Sender<()>>,
}

impl Listening {
  /// Binds the loopback port and serves `handler` on `runtime` to clients presenting `token`.
  pub fn start(runtime: &Handle, handler: Handler, token: BearerToken) -> io::Result<Self> {
    let listener = TcpListener::bind((LOOPBACK, 0))?;
    listener.set_nonblocking(true)?;
    let port = NonZeroU16::new(listener.local_addr()?.port()).ok_or_else(|| io::Error::other("the OS bound port 0"))?;
    let (stop, stopped) = oneshot::channel();
    // The server stops when told to (`Drop`), finishing the requests under way.
    drop(runtime.spawn(serve(listener, handler, token, stopped)));
    Ok(Self { port, stop: Some(stop) })
  }

  pub const fn port(&self) -> NonZeroU16 {
    self.port
  }
}

impl Drop for Listening {
  fn drop(&mut self) {
    if let Some(stop) = self.stop.take()
      && stop.send(()).is_err()
    {
      tracing::debug!("Orchester's MCP server had ended already");
    }
  }
}

async fn serve(listener: TcpListener, handler: Handler, token: BearerToken, stopped: oneshot::Receiver<()>) {
  let listener = match tokio::net::TcpListener::from_std(listener) {
    Ok(listener) => listener,
    Err(error) => {
      tracing::error!(%error, "Orchester's MCP server cannot listen");
      return;
    }
  };
  let config = StreamableHttpServerConfig::default();
  let sessions = config.cancellation_token.clone();
  let service = StreamableHttpService::new(
    move || Ok(handler.clone()),
    Arc::new(LocalSessionManager::default()),
    config,
  );
  let router = Router::new()
    .route_service(PATH, service)
    .layer(middleware::from_fn_with_state(token, authorize));
  let shutdown = async move {
    // Stopped, or its owner is gone without saying so: the server stops either way.
    drop(stopped.await);
    sessions.cancel();
  };
  if let Err(error) = axum::serve(listener, router).with_graceful_shutdown(shutdown).await {
    tracing::error!(%error, "Orchester's MCP server stopped");
  }
}

/// Lets a request through only with the bearer token.
async fn authorize(State(token): State<BearerToken>, request: Request, next: Next) -> Response {
  let presented = request
    .headers()
    .get(AUTHORIZATION)
    .and_then(|value| value.to_str().ok())
    .and_then(|value| value.strip_prefix("Bearer "));
  if presented.is_some_and(|presented| token.admits(presented)) {
    return next.run(request).await;
  }
  (
    StatusCode::UNAUTHORIZED,
    "missing or invalid bearer token; read it from Orchester's MCP discovery file",
  )
    .into_response()
}
