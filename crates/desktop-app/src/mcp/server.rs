//! The HTTP side of the app's MCP server: streamable HTTP at `/mcp` on `127.0.0.1`, on the installed app's fixed
//! port or one the OS picks, on the app's runtime. Every request must carry the bearer token and name the loopback
//! address in its `Host`; a request without both is refused before the MCP layer reads it (`guard`). Each client
//! session gets its own `McpService` of lurq's hosted tools, the app's included. Changing the token ends the
//! sessions and keeps the port. Adapted from the deprecated desktop's `app_mcp::server`, which goes with that crate.

use std::{
  io,
  net::{Ipv4Addr, SocketAddr, TcpListener},
  num::NonZeroU16,
  sync::{Arc, mpsc as std_mpsc},
  time::Duration,
};

use axum::{Router, middleware};
use hyper::server::conn::http1;
use hyper_util::{
  rt::{TokioIo, TokioTimer},
  service::TowerToHyperService,
};
use lurq::mcp::{
  McpHandle,
  rmcp::{
    ServerHandler,
    transport::streamable_http_server::{
      StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
  },
};
use tokio::{runtime::Handle, sync::mpsc, task::JoinSet};

use super::{
  bind,
  clients::Clients,
  guard::{self, Admission},
  input_gate::InputGate,
  status::Taken,
  token::BearerToken,
};

/// The only address the server listens on.
const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;
const PATH: &str = "/mcp";
/// How long a client may take to send a request's headers, and how long a kept-alive connection idles.
const HEADER_TIMEOUT: Duration = Duration::from_secs(10);
/// Connections served at once; local clients need a few.
const MAX_CONNECTIONS: usize = 32;
/// How long stopping waits for the port to be let go, so the same port can be taken again at once.
const STOP_WAIT: Duration = Duration::from_secs(2);

/// `http://127.0.0.1:<port>/mcp`, the endpoint clients connect to.
pub fn endpoint(port: NonZeroU16) -> String {
  format!("http://{}{PATH}", address(port))
}

/// `127.0.0.1:<port>`.
pub fn address(port: NonZeroU16) -> SocketAddr {
  SocketAddr::from((LOOPBACK, port.get()))
}

/// Why the server could not listen.
#[derive(Debug, thiserror::Error)]
pub enum BindError {
  /// The port is kept from the server.
  #[error("the port is kept from the server: {0:?}")]
  Taken(Taken),
  #[error(transparent)]
  Other(#[from] io::Error),
}

/// What every session of the server is served with.
#[derive(Clone)]
pub struct Serving {
  pub gate: InputGate,
  pub tools: McpHandle,
  pub clients: Clients,
}

enum Control {
  /// Clients must present this token from now on; the sessions of the old one end.
  Rotate(BearerToken),
  /// Stop, and say so once the port is let go.
  Stop(std_mpsc::Sender<()>),
}

/// The server, listening; dropping it stops it.
pub struct Listening {
  port: NonZeroU16,
  control: mpsc::UnboundedSender<Control>,
}

impl Listening {
  /// Binds `port` on loopback (`None`: one the OS picks) and serves the sessions of `serving` on `runtime` to
  /// clients presenting `token`.
  pub fn start(
    runtime: &Handle,
    port: Option<NonZeroU16>,
    serving: Serving,
    token: BearerToken,
  ) -> Result<Self, BindError> {
    let Serving { gate, tools, clients } = serving;
    // One service per client session: input a client holds is released when its session ends.
    let sessions = move || gate.guard_listed(tools.service(), clients.session());
    Self::serve_sessions(runtime, port, sessions, token)
  }

  /// Binds `port` on loopback (`None`: one the OS picks) and serves each session of a client presenting `token`
  /// with the service `sessions` makes for it, on `runtime`.
  pub(super) fn serve_sessions<S>(
    runtime: &Handle,
    port: Option<NonZeroU16>,
    sessions: impl Fn() -> S + Clone + Send + Sync + 'static,
    token: BearerToken,
  ) -> Result<Self, BindError>
  where
    S: ServerHandler,
  {
    let address = SocketAddr::from((LOOPBACK, port.map_or(0, NonZeroU16::get)));
    let listener = bind::listen(address).map_err(|error| match Taken::of_bind_error(&error) {
      Some(how) => BindError::Taken(how),
      None => BindError::Other(error),
    })?;
    listener.set_nonblocking(true)?;
    let port = NonZeroU16::new(listener.local_addr()?.port()).ok_or_else(|| io::Error::other("the OS bound port 0"))?;
    let (control, controlled) = mpsc::unbounded_channel();
    // The server stops when told to, or when its owner is gone: it ends its sessions and drops the connections
    // open.
    drop(runtime.spawn(serve(listener, port, sessions, token, controlled)));
    Ok(Self { port, control })
  }

  pub const fn port(&self) -> NonZeroU16 {
    self.port
  }

  /// From now on clients must present `token`; the sessions open end.
  pub fn rotate(&self, token: BearerToken) {
    if self.control.send(Control::Rotate(token)).is_err() {
      tracing::warn!("the app's MCP server had ended before its token changed");
    }
  }

  /// Stops the server and waits, briefly, until its port is let go. It blocks: never called from async code.
  pub fn stop(self) {
    let (stopped, wait) = std_mpsc::channel();
    if self.control.send(Control::Stop(stopped)).is_err() {
      return;
    }
    if wait.recv_timeout(STOP_WAIT).is_err() {
      tracing::warn!("the app's MCP server did not say it stopped in time");
    }
  }
}

async fn serve<S>(
  listener: TcpListener,
  port: NonZeroU16,
  sessions: impl Fn() -> S + Clone + Send + Sync + 'static,
  token: BearerToken,
  mut controlled: mpsc::UnboundedReceiver<Control>,
) where
  S: ServerHandler,
{
  let listener = match tokio::net::TcpListener::from_std(listener) {
    Ok(listener) => listener,
    Err(error) => {
      tracing::error!(%error, "the app's MCP server cannot listen");
      return;
    }
  };
  let (mut router, mut end_sessions) = routes(sessions.clone(), port, token);
  let mut connections = JoinSet::new();
  let stopped = loop {
    tokio::select! {
      control = controlled.recv() => match control {
        Some(Control::Rotate(token)) => {
          end_sessions();
          connections.abort_all();
          (router, end_sessions) = routes(sessions.clone(), port, token);
        }
        Some(Control::Stop(stopped)) => break Some(stopped),
        // Its owner is gone without saying so: the server stops.
        None => break None,
      },
      accepted = listener.accept() => match accepted {
        // Past the cap the connection is closed at once rather than queued behind idle ones.
        Ok((stream, _)) if connections.len() < MAX_CONNECTIONS => {
          connections.spawn(connection(stream, router.clone()));
        }
        Ok(_) => tracing::warn!("the app's MCP server refused a connection over its limit"),
        Err(error) => tracing::warn!(%error, "the app's MCP server could not accept a connection"),
      },
      Some(ended) = connections.join_next() => {
        if let Err(error) = ended
          && !error.is_cancelled()
        {
          tracing::error!(%error, "an MCP connection's task failed");
        }
      }
    }
  };
  end_sessions();
  connections.abort_all();
  drop(listener);
  // Whoever waits for the port hears it is free; one that stopped waiting needs no answer.
  if let Some(stopped) = stopped
    && stopped.send(()).is_err()
  {
    tracing::debug!("nobody waited for the app's MCP server to stop");
  }
}

/// Ends the sessions of one token.
type EndSessions = Box<dyn FnOnce() + Send>;

/// The routes of one token: its sessions, behind the guard, and how to end them.
fn routes<S>(
  sessions: impl Fn() -> S + Send + Sync + 'static,
  port: NonZeroU16,
  token: BearerToken,
) -> (Router, EndSessions)
where
  S: ServerHandler,
{
  let config = StreamableHttpServerConfig::default();
  let ending = config.cancellation_token.clone();
  let service = StreamableHttpService::new(move || Ok(sessions()), Arc::new(LocalSessionManager::default()), config);
  let router = Router::new()
    .route_service(PATH, service)
    .layer(middleware::from_fn_with_state(
      Admission::new(token, port),
      guard::admit,
    ));
  (router, Box::new(move || ending.cancel()))
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
