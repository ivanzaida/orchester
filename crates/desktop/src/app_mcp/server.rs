//! The HTTP side of the server: streamable HTTP at `/mcp` on `127.0.0.1`, on a port the OS picks, on
//! the application's runtime. Every request must carry the bearer token of the discovery file; a
//! request without it is refused before the MCP layer reads it.

use std::{
  io,
  net::{Ipv4Addr, SocketAddr, TcpListener},
  num::NonZeroU16,
  sync::Arc,
  time::Duration,
};

use axum::{Router, middleware};
use hyper::server::conn::http1;
use hyper_util::{
  rt::{TokioIo, TokioTimer},
  service::TowerToHyperService,
};
use rmcp::transport::streamable_http_server::{
  StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::{runtime::Handle, sync::oneshot, task::JoinSet};

use super::{guard, handler::Handler};

/// The only address the server listens on.
const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;
const PATH: &str = "/mcp";
/// How long a client may take to send a request's headers, and how long a kept-alive connection idles.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections served at once; local clients need a few.
const MAX_CONNECTIONS: usize = 32;

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
  pub(super) fn admits(&self, presented: &str) -> bool {
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

async fn serve(listener: TcpListener, handler: Handler, token: BearerToken, mut stopped: oneshot::Receiver<()>) {
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
    .layer(middleware::from_fn_with_state(token, guard::admit));
  let mut connections = JoinSet::new();
  loop {
    tokio::select! {
      // Stopped, or its owner is gone without saying so: the server stops either way.
      _ = &mut stopped => break,
      accepted = listener.accept() => match accepted {
        // Past the cap the connection is closed at once rather than queued behind idle ones.
        Ok((stream, _)) if connections.len() < MAX_CONNECTIONS => {
          connections.spawn(connection(stream, router.clone()));
        }
        Ok(_) => tracing::warn!("Orchester's MCP server refused a connection over its limit"),
        Err(error) => tracing::warn!(%error, "Orchester's MCP server could not accept a connection"),
      },
      Some(ended) = connections.join_next() => {
        if let Err(error) = ended {
          tracing::error!(%error, "an MCP connection's task failed");
        }
      }
    }
  }
  sessions.cancel();
  connections.abort_all();
}

/// Serves one connection. The header deadline starts on accept and again after each response, so it
/// also bounds how long a kept-alive connection idles; a response that streams events is not cut.
async fn connection(stream: tokio::net::TcpStream, router: Router) {
  let mut builder = http1::Builder::new();
  builder.timer(TokioTimer::new()).header_read_timeout(HEADER_TIMEOUT);
  let served = builder
    .serve_connection(TokioIo::new(stream), TowerToHyperService::new(router))
    .await;
  if let Err(error) = served {
    // Hyper's text can quote the request; only its kind is logged.
    tracing::debug!(
      timeout = error.is_timeout(),
      parse = error.is_parse(),
      "an MCP connection ended"
    );
  }
}
