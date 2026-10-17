use std::{future::Future, net::SocketAddr, time::Duration};

use axum::{Extension, Router, extract::ConnectInfo, serve::Listener};
use hyper::server::conn::http1;
use hyper_util::{
  rt::{TokioIo, TokioTimer},
  service::TowerToHyperService,
};
use tokio::{net::TcpListener, sync::watch, task::JoinSet};

use crate::{
  Server,
  connection_limit::ConnectionLimit,
  error::ServerError,
  initial_read::InitialRead,
  tls::{PeerAddr, TlsListener},
};

#[derive(Clone)]
pub(crate) struct ConnectionStop(pub watch::Receiver<bool>);

/// Serves a real bound listener with bounded connections and HTTP read deadlines, including direct TLS.
pub async fn serve(
  server: Server,
  listener: TcpListener,
  shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ServerError> {
  let tls = server.inner.tls.as_ref().map(|tls| tls.acceptor.clone());
  let proxies = server.inner.config.trusted_proxies.clone();
  let limits = ConnectionLimit::new(256, 64, proxies.clone());
  let schedule = server.inner.presented.start_schedule();
  let app = crate::http::router(server);
  match tls {
    Some(acceptor) => serve_listener(TlsListener::new(listener, acceptor, proxies), app, shutdown, limits).await,
    None => serve_listener(listener, app, shutdown, limits).await,
  }
  if let Some(schedule) = schedule {
    schedule.abort();
  }
  Ok(())
}

/// Serves the private listener the server admin app's own server side calls the API on (`admin_api_listen`): plain
/// HTTP on a private address, the API only, the browser's address and host taken from the admin's forwarded headers.
pub async fn serve_admin_api(
  server: Server,
  listener: TcpListener,
  shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), ServerError> {
  let limits = ConnectionLimit::new(256, 256, Vec::new());
  serve_listener(listener, crate::http::internal_router(server), shutdown, limits).await;
  Ok(())
}

async fn serve_listener<L: Listener<Addr = SocketAddr>>(
  mut listener: L,
  app: Router,
  shutdown: impl Future<Output = ()>,
  limits: ConnectionLimit,
) {
  let mut connections = JoinSet::new();
  let (stop, stopped) = watch::channel(false);
  tokio::pin!(shutdown);
  loop {
    tokio::select! {
      _ = &mut shutdown => break,
      (stream, address) = listener.accept() => {
        // Reject excess connections immediately instead of queuing a different peer behind idle sockets.
        if connections.len() >= 256 { continue; }
        let Some(lease) = limits.acquire(address.ip()) else { continue; };
        let service = TowerToHyperService::new(app.clone()
          .layer(Extension(ConnectInfo(PeerAddr(address))))
          .layer(Extension(ConnectionStop(stopped.clone()))));
        let stopped = stopped.clone();
        connections.spawn(async move {
          let _lease = lease;
          let mut builder = http1::Builder::new();
          // The header timer starts on accept and again after each response, bounding keep-alive idle time too.
          builder.timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(10));
          let connection = builder.serve_connection(TokioIo::new(InitialRead::new(stream)), service);
          tokio::pin!(connection);
          let mut stopped = stopped;
          let result = tokio::select! {
            result = &mut connection => Some(result),
            changed = stopped.changed() => {
              if changed.is_err() { tracing::info!("server shutdown channel closed"); }
              connection.as_mut().graceful_shutdown();
              match tokio::time::timeout(Duration::from_secs(15), &mut connection).await {
                Ok(result) => Some(result),
                Err(_) => { tracing::warn!("connection shutdown deadline reached"); None }
              }
            }
          };
          if let Some(Err(error)) = result { log_connection_error(&error); }
        });
      }
      Some(finished) = connections.join_next() => {
        if finished.is_err() { tracing::error!(error_kind = "Worker", "connection worker failed"); }
      }
    }
  }
  drop(listener);
  stop.send_replace(true);
  while let Some(finished) = connections.join_next().await {
    if finished.is_err() {
      tracing::error!(error_kind = "Worker", "connection worker failed");
    }
  }
}

fn log_connection_error(error: &hyper::Error) {
  // Hyper's source text can include request values; log only its structural classification.
  let kind = if error.is_timeout() {
    "Timeout"
  } else if error.is_parse() {
    "Parse"
  } else if error.is_incomplete_message() {
    "IncompleteMessage"
  } else if error.is_closed() {
    "Closed"
  } else {
    "Transport"
  };
  tracing::warn!(error_kind = kind, "HTTP connection failed");
}
