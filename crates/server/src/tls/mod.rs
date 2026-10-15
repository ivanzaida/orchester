pub(crate) mod certificate;

use std::{io, net::SocketAddr, sync::Arc, time::Duration};

use axum::serve::Listener;
use orchester_api::certificate::CertificatePin;
use tokio::{
  net::{TcpListener, TcpStream},
  task::JoinSet,
};
use tokio_rustls::{TlsAcceptor, rustls, server::TlsStream};

use crate::{
  addresses::TrustedProxy, config::Config, connection_limit::ConnectionLimit, error::ServerError,
  initial_read::InitialRead,
};

type Stream = TlsStream<InitialRead<TcpStream>>;

#[derive(Clone, Copy)]
pub(crate) struct PeerAddr(pub SocketAddr);

/// The server's own TLS: the acceptor and the pin of the certificate it presents.
#[derive(Clone)]
pub(crate) struct Tls {
  pub acceptor: TlsAcceptor,
  pub pin: CertificatePin,
}

/// The TLS `config` asks for, the certificate generated first when the server has none yet; `None` for plain HTTP.
pub(crate) fn load(config: &Config, now_ms: i64) -> Result<Option<Tls>, ServerError> {
  let Some(material) = certificate::load(config, now_ms)? else {
    return Ok(None);
  };
  let provider = Arc::new(rustls::crypto::ring::default_provider());
  let mut tls = rustls::ServerConfig::builder_with_provider(provider)
    .with_safe_default_protocol_versions()
    .map_err(|_| ServerError::Tls)?
    .with_no_client_auth()
    .with_single_cert(material.chain, material.key)
    .map_err(|_| ServerError::Tls)?;
  tls.alpn_protocols = vec![b"http/1.1".to_vec()];
  Ok(Some(Tls {
    acceptor: TlsAcceptor::from(Arc::new(tls)),
    pin: material.pin,
  }))
}

pub(crate) struct TlsListener {
  listener: TcpListener,
  acceptor: TlsAcceptor,
  handshakes: JoinSet<Result<(Stream, SocketAddr), ServerError>>,
  limits: ConnectionLimit,
}

impl TlsListener {
  pub fn new(listener: TcpListener, acceptor: TlsAcceptor, trusted_proxies: Vec<TrustedProxy>) -> Self {
    Self {
      listener,
      acceptor,
      handshakes: JoinSet::new(),
      limits: ConnectionLimit::new(64, 32, trusted_proxies),
    }
  }
}

impl Listener for TlsListener {
  type Io = Stream;
  type Addr = SocketAddr;

  async fn accept(&mut self) -> (Self::Io, Self::Addr) {
    loop {
      tokio::select! {
        accepted = self.listener.accept() => {
          match accepted {
            Ok((stream, address)) => {
              if self.handshakes.len() >= 64 { continue; }
              let Some(lease) = self.limits.acquire(address.ip()) else { continue; };
              let acceptor = self.acceptor.clone();
              self.handshakes.spawn(async move {
                let _lease = lease;
                let stream = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(InitialRead::new(stream))).await
                  .map_err(|_| ServerError::Tls)?.map_err(|_| ServerError::Tls)?;
                Ok((stream, address))
              });
            }
            Err(_) => {
              tracing::warn!("cannot accept server connection");
              tokio::time::sleep(Duration::from_millis(100)).await;
            }
          }
        }
        finished = self.handshakes.join_next(), if !self.handshakes.is_empty() => {
          match finished {
            Some(Ok(Ok(connection))) => return connection,
            _ => tracing::warn!("TLS handshake failed"),
          }
        }
      }
    }
  }

  fn local_addr(&self) -> io::Result<Self::Addr> {
    self.listener.local_addr()
  }
}
