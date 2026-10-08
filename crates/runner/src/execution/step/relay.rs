//! The runner's end of the relay an invocation's CLI starts (`crate::approval_relay`): a loopback port of
//! the runner's own, which the relay connects to with the invocation's token. Over that connection the
//! relay hands over Claude Code's permission prompt calls ([`super::claude_approval`]) and the calls of the
//! step's Orchester tools ([`super::orchester_tools`]), one [`RelayRequest`] per line, each answered on
//! one line. The listener and its connection end with the worker.
//!
//! A connection must show the token on a short first line within [`HANDSHAKE`] (compared in constant time);
//! after [`ATTEMPTS`] that do not, nothing more is accepted. Claude Code starts its permission prompt once,
//! so only the first relay that shows the token is served, and no connection is taken after it. Codex
//! 0.159.2 starts Orchester's server once for the thread and once more for each `mcpServerStatus/list`,
//! which lists its tools and closes it, at about the same time (checked 2026-10-08 with the real relay and
//! `codex app-server`): a listener that served only the first would leave the thread's relay, or the
//! list's, waiting until Codex gives up on it (*timed out awaiting tools/list*). So every Codex relay that
//! shows the token is served, each on its own connection; none of them gets more than another, as the
//! runner itself asks about every Codex call of a tool that asks.
//! While a prompt call waits for a person, its connection is watched: a relay that goes away ends the
//! wait, so a later decision cannot be recorded as delivered.

use std::{
  future::pending,
  net::{Ipv4Addr, SocketAddr},
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  connection::claude_approval::{
    ApprovalToken, ClaudeApprovalRequest, ClaudeApprovalScope, ClaudeAsk, refusal, relay::RelayRequest,
  },
  orchester_tools::{OrchesterTool, SERVER, ToolResult},
};
use serde_json::Value;
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
  net::{
    TcpListener,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
  },
  sync::{mpsc, oneshot},
  task::{JoinHandle, JoinSet},
  time::timeout,
};

use super::{
  claude_approval::{Asked, AskedBy, PromptAnswer},
  orchester_tools::ToolHost,
};

/// How long a connection has to show the token.
const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long the listener waits after a failed accept before the next.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);
/// Connections that could not be taken or did not show the token before the listener gives up.
const ATTEMPTS: usize = 8;
/// The longest first line read: the token is 64 characters.
const TOKEN_LINE: u64 = 128;
/// What the agent is told of a call of Orchester's tools nobody answered.
const DENIED: &str = "Denied in Orchester.";

/// What one invocation's relay is served.
pub(super) struct Served {
  /// Claude Code's permission prompt: what it may leave to a person. `None` for Codex.
  pub(super) prompt: Option<ClaudeApprovalScope>,
  pub(super) tools: Arc<ToolHost>,
}

/// A listener waiting for its relay, ended when this is dropped.
pub(super) struct Listening {
  /// The prompt calls a person decides.
  requests: mpsc::UnboundedReceiver<Asked>,
  /// Set once the relay showed the token.
  reached: Arc<AtomicBool>,
  listener: JoinHandle<()>,
}

impl Drop for Listening {
  fn drop(&mut self) {
    self.listener.abort();
  }
}

impl Listening {
  /// The next call a person decides; never ready without a listener.
  pub(super) async fn next(listening: Option<&mut Self>) -> Option<Asked> {
    let Some(listening) = listening else {
      return pending().await;
    };
    match listening.requests.recv().await {
      Some(asked) => Some(asked),
      None => pending().await,
    }
  }

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
      asked: AtomicU64::new(0),
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
  /// Calls of Orchester's tools asked about so far, which name each such request.
  asked: AtomicU64,
}

impl Relay {
  /// Serves the connections that show the token (only the first for Claude Code's permission prompt),
  /// until [`ATTEMPTS`] could not be taken or did not show it, or the worker ended. The
  /// connections end with this task: aborting it drops `served`, which aborts them.
  async fn accept(self, listener: TcpListener) {
    let relay = Arc::new(self);
    let mut served = JoinSet::new();
    let mut failed = 0;
    while failed < ATTEMPTS {
      let stream = tokio::select! {
        accepted = listener.accept() => match accepted {
          Ok((stream, _)) => stream,
          Err(error) => {
            tracing::warn!(%error, "the relay's listener cannot accept its relay");
            failed += 1;
            // Without a pause, an accept that keeps failing would spin.
            tokio::time::sleep(ACCEPT_RETRY).await;
            continue;
          }
        },
        () = relay.requests.closed() => return,
      };
      let (read, write) = stream.into_split();
      let mut reader = BufReader::new(read);
      if !relay.shows_token(&mut reader).await {
        tracing::warn!("a connection to the relay's listener did not show the invocation's token");
        failed += 1;
        continue;
      }
      relay.reached.store(true, Ordering::Release);
      if relay.served.prompt.is_some() {
        // No other connection is taken while or after Claude Code's permission prompt is served.
        drop(listener);
        return relay.serve(reader.lines(), write).await;
      }
      let relay = relay.clone();
      served.spawn(async move { relay.serve(reader.lines(), write).await });
      // Connections that ended are let go of, so a long step does not keep them all.
      while served.try_join_next().is_some() {}
    }
    tracing::warn!("the relay's listener stopped listening after {ATTEMPTS} failed connections");
    relay.requests.closed().await;
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
        Ok(RelayRequest::Ask(arguments)) => self.decide(&arguments, &mut lines).await,
        Ok(RelayRequest::List) => Some(Value::Array(self.served.tools.listed()).to_string()),
        Ok(RelayRequest::Call { name, arguments }) => self
          .call(&name, &arguments, &mut lines)
          .await
          .map(|result| result.mcp().to_string()),
        Err(_) => Some(refusal()),
      };
      // `None`: the relay went away while a call waited for a person; the worker's wait ends with it.
      let Some(answer) = answer else {
        return;
      };
      if write.write_all(format!("{answer}\n").as_bytes()).await.is_err() {
        return;
      }
    }
  }

  /// The decision on permission prompt call `arguments`: a person's, or a refusal of what the step does
  /// not leave to one; `None` when the relay closed its connection while the call waited. An approved call
  /// of one of Orchester's tools leaves a grant for that call.
  async fn decide(&self, arguments: &Value, lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Option<String> {
    let asked = self.served.prompt.as_ref().map(|scope| scope.classify(arguments));
    let Some(ClaudeAsk::Person(request)) = asked else {
      return Some(refusal());
    };
    let tool = (request.server() == SERVER)
      .then(|| OrchesterTool::from_name(request.action()))
      .flatten();
    let answer = self.ask(request, AskedBy::Prompt, lines).await?;
    if let (Some(tool), PromptAnswer::Allow) = (tool, &answer) {
      self
        .served
        .tools
        .grant(tool, arguments.get("input").unwrap_or(&Value::Null));
    }
    Some(answer.result())
  }

  /// The answer to a call of Orchester's tool `name`: refused when it is not valid or not offered, asked
  /// about first when it needs an approval and has none; `None` when the relay closed its connection while
  /// the call waited.
  async fn call(
    &self,
    name: &str,
    arguments: &Value,
    lines: &mut Lines<BufReader<OwnedReadHalf>>,
  ) -> Option<ToolResult> {
    let tools = &self.served.tools;
    let call = match tools.parse(name, arguments) {
      Ok(call) => call,
      Err(error) => return Some(ToolResult::refused(&error)),
    };
    if tools.asks(call.tool()) && !tools.take_grant(call.tool(), arguments) {
      let number = self.asked.fetch_add(1, Ordering::Relaxed);
      let request =
        ClaudeApprovalRequest::tool_call(format!("orchester-call-{number}"), name, arguments, tools.secrets());
      match self.ask(request, AskedBy::ToolCall, lines).await? {
        PromptAnswer::Allow => {}
        PromptAnswer::Deny(told) => return Some(ToolResult::unavailable(&told)),
        PromptAnswer::Repeated => return Some(ToolResult::unavailable(DENIED)),
      }
    }
    Some(tools.run(call).await)
  }

  /// The person's answer on `request`; a repeated call's denial when the worker ended without one; `None`
  /// when the relay closed its connection or broke the protocol while the call waited.
  async fn ask(
    &self,
    request: ClaudeApprovalRequest,
    by: AskedBy,
    lines: &mut Lines<BufReader<OwnedReadHalf>>,
  ) -> Option<PromptAnswer> {
    let (reply, decision) = oneshot::channel();
    if self.requests.send(Asked { request, reply, by }).is_err() {
      return Some(PromptAnswer::Repeated);
    }
    tokio::select! {
      // A dropped sender is a worker that ended without a decision: the call is denied.
      decision = decision => Some(decision.unwrap_or(PromptAnswer::Repeated)),
      // The relay sends nothing while it waits; anything, or the end of the stream, ends the wait.
      _ = lines.next_line() => None,
    }
  }
}
