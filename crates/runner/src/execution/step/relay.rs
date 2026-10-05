//! The runner's end of the relay an invocation's CLI starts (`crate::approval_relay`): a loopback port of
//! the runner's own, which the relay connects to with the invocation's token. Over that connection the
//! relay hands over Claude Code's permission prompt calls ([`super::claude_approval`]) and the calls of the
//! step's Orchester tools ([`super::orchester_tools`]), one [`RelayRequest`] per line, each answered on
//! one line. The listener and its connection end with the worker.
//!
//! Only one relay is served: connections are taken one at a time, each must show the token on a short
//! first line within [`HANDSHAKE`] (compared in constant time), and after the first that does, or after
//! [`ATTEMPTS`] that do not, nothing more is accepted. While a prompt call waits for a person, the
//! connection is watched: a relay that goes away ends the wait, so a later decision cannot be recorded as
//! delivered.

use std::{
  net::{Ipv4Addr, SocketAddr},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_core::connection::claude_approval::{
  ApprovalToken, ClaudeApprovalRequest, ClaudeApprovalScope, ClaudeAsk, not_answered, refusal, relay::RelayRequest,
};
use serde_json::Value;
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
  net::{
    TcpListener,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
  },
  sync::{mpsc, oneshot},
  task::JoinHandle,
  time::timeout,
};

use super::{claude_approval::Asked, orchester_tools::ToolHost};

/// How long a connection has to show the token.
const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long the listener waits after a failed accept before the next.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);
/// Connections without the token before the listener gives up.
const ATTEMPTS: usize = 8;
/// The longest first line read: the token is 64 characters.
const TOKEN_LINE: u64 = 128;

/// What one invocation's relay is served.
pub(super) struct Served {
  /// Claude Code's permission prompt: what it may leave to a person. `None` for Codex.
  pub(super) prompt: Option<ClaudeApprovalScope>,
  pub(super) tools: Arc<ToolHost>,
}

/// A listener waiting for its relay, ended when this is dropped.
pub(super) struct Listening {
  /// The prompt calls a person decides.
  pub(super) requests: mpsc::UnboundedReceiver<Asked>,
  /// Set once the relay showed the token.
  pub(super) reached: Arc<AtomicBool>,
  listener: JoinHandle<()>,
}

impl Drop for Listening {
  fn drop(&mut self) {
    self.listener.abort();
  }
}

impl Listening {
  /// Whether the relay reached the runner.
  pub(super) fn reached(&self) -> bool {
    self.reached.load(Ordering::Acquire)
  }
}

/// A loopback port, bound before the relay's configuration names it.
pub(super) struct Bound {
  listener: TcpListener,
  address: String,
}

/// Binds a free loopback port.
pub(super) fn bind() -> std::io::Result<Bound> {
  let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
  listener.set_nonblocking(true)?;
  let address = listener.local_addr()?.to_string();
  Ok(Bound {
    listener: TcpListener::from_std(listener)?,
    address,
  })
}

impl Bound {
  /// `127.0.0.1:<port>`, for the relay's configuration.
  pub(super) fn address(&self) -> &str {
    &self.address
  }

  /// Waits for the relay that shows `token`, and serves it `served`.
  pub(super) fn serve(self, served: Served, token: ApprovalToken) -> Listening {
    let (sender, requests) = mpsc::unbounded_channel();
    let reached = Arc::new(AtomicBool::new(false));
    let relay = Relay {
      token,
      served,
      requests: sender,
      reached: reached.clone(),
    };
    Listening {
      requests,
      reached,
      listener: tokio::spawn(relay.accept(self.listener)),
    }
  }
}

/// What one invocation's listener serves.
struct Relay {
  token: ApprovalToken,
  served: Served,
  requests: mpsc::UnboundedSender<Asked>,
  reached: Arc<AtomicBool>,
}

impl Relay {
  /// Takes connections one at a time until one shows the token, then serves only that one.
  async fn accept(self, listener: TcpListener) {
    for _ in 0..ATTEMPTS {
      let stream = tokio::select! {
        accepted = listener.accept() => match accepted {
          Ok((stream, _)) => stream,
          Err(error) => {
            tracing::warn!(%error, "the relay's listener cannot accept its relay");
            // Without a pause, an accept that keeps failing would spin.
            tokio::time::sleep(ACCEPT_RETRY).await;
            continue;
          }
        },
        () = self.requests.closed() => return,
      };
      let (read, write) = stream.into_split();
      let mut reader = BufReader::new(read);
      if !self.shows_token(&mut reader).await {
        tracing::warn!("a connection to the relay's listener did not show the invocation's token");
        continue;
      }
      self.reached.store(true, Ordering::Release);
      // No other connection is taken while or after this one is served.
      drop(listener);
      return self.serve(reader.lines(), write).await;
    }
    tracing::warn!("the relay's listener stopped listening after {ATTEMPTS} connections without its token");
  }

  async fn shows_token(&self, reader: &mut BufReader<OwnedReadHalf>) -> bool {
    let mut line = String::new();
    let read = timeout(HANDSHAKE, reader.take(TOKEN_LINE).read_line(&mut line)).await;
    matches!(read, Ok(Ok(length)) if length > 0) && self.token.matches(line.trim_end())
  }

  /// One request per line, each answered on one line.
  async fn serve(&self, mut lines: Lines<BufReader<OwnedReadHalf>>, mut write: OwnedWriteHalf) {
    while let Ok(Some(line)) = lines.next_line().await {
      let answer = match serde_json::from_str::<RelayRequest>(&line) {
        Ok(RelayRequest::Ask(arguments)) => match self.decide(&arguments, &mut lines).await {
          Some(decision) => decision,
          // The relay went away while the call waited: the worker's wait ends with it.
          None => return,
        },
        Ok(RelayRequest::List) => Value::Array(self.served.tools.listed()).to_string(),
        Ok(RelayRequest::Call { name, arguments }) => self.served.tools.call(&name, &arguments).await.mcp().to_string(),
        Err(_) => refusal(),
      };
      if write.write_all(format!("{answer}\n").as_bytes()).await.is_err() {
        return;
      }
    }
  }

  /// The decision on permission prompt call `arguments`: a person's, or a refusal of what the step does
  /// not leave to one; `None` when the relay closed its connection while the call waited.
  async fn decide(&self, arguments: &Value, lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Option<String> {
    let asked = self.served.prompt.as_ref().map(|scope| scope.classify(arguments));
    match asked {
      Some(ClaudeAsk::Person(request)) => self.ask(request, lines).await,
      Some(ClaudeAsk::Refused) | None => Some(refusal()),
    }
  }

  /// The person's decision on `request`; a denial when the worker ended without one; `None` when
  /// the relay closed its connection or broke the protocol while the call waited.
  async fn ask(&self, request: ClaudeApprovalRequest, lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Option<String> {
    let (reply, decision) = oneshot::channel();
    if self.requests.send(Asked { request, reply }).is_err() {
      return Some(not_answered());
    }
    tokio::select! {
      // A dropped sender is a worker that ended without a decision: the call is denied.
      decision = decision => Some(decision.map_or_else(|_| not_answered(), |answer| answer.result())),
      // The relay sends nothing while it waits; anything, or the end of the stream, ends the wait.
      _ = lines.next_line() => None,
    }
  }
}
