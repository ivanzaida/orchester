//! The runner's end of the relay an invocation's CLI starts (`crate::approval_relay`): a loopback port of
//! the runner's own, which the relay connects to with the invocation's token. Over that connection the
//! relay hands over Claude Code's permission prompt calls ([`super::claude_approval`]), the calls of the
//! step's Orchester tools ([`super::orchester_tools`]) and, for Claude Code, its `ask_question` calls
//! ([`super::question`]), one [`RelayRequest`] per line, each answered on one line. The listener and its
//! connection end with the worker.
//!
//! A connection must show the token on a short first line within [`HANDSHAKE`] (compared in constant time),
//! each connection on its own, so a silent one holds no other back; at most [`HANDSHAKES`] are read at once,
//! and connections beyond them wait in the port's backlog. A connection without the token never closes the
//! port, so none can keep the CLI's relays out. Only the relays the CLI starts are served, each on its own
//! connection ([`Served::relays`]), and the port closes once they have shown the token: Claude Code starts
//! its permission prompt once; Codex 0.159.2 starts Orchester's server once for the thread and once more for
//! the runner's one `mcpServerStatus/list`, which lists its tools and closes it, at about the same time
//! ([`CODEX_RELAYS`]; checked 2026-10-08 with the real relay and `codex app-server`). A listener that served
//! only the first would leave the thread's relay, or the list's, waiting until Codex gives up on it (*timed
//! out awaiting tools/list*). Once Codex's inventory was checked, the port waits at most [`RELAY_WAIT`] for
//! the relays still to come ([`Listening::stop_accepting`]); a handshake under way when it closes still
//! counts, and the worker says in the activity when fewer came ([`Listening::taken`]).
//! While a prompt call waits for a person, its connection is watched: a relay that goes away ends the
//! wait, so a later decision cannot be recorded as delivered.

use std::{
  future::pending,
  net::{Ipv4Addr, SocketAddr},
  sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  connection::claude_approval::{
    ApprovalToken, ClaudeApprovalRequest, ClaudeApprovalScope, ClaudeAsk, refusal, relay::RelayRequest,
  },
  orchester_tools::{OrchesterTool, SERVER, ToolResult},
  question::tool as ask_question,
};
use serde_json::Value;
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
  net::{
    TcpListener, TcpStream,
    tcp::{OwnedReadHalf, OwnedWriteHalf},
  },
  sync::{mpsc, oneshot},
  task::{JoinError, JoinHandle, JoinSet},
  time::{Instant, sleep_until, timeout},
};

use super::{
  claude_approval::{Asked, AskedBy, PromptAnswer},
  orchester_tools::ToolHost,
  question::AskedQuestion,
};

/// How long a connection has to show the token.
const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long the listener waits after a failed accept before the next.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);
/// Connections read for the token at once.
const HANDSHAKES: usize = 8;
/// How long the port stays open for the relays still to come once Codex's inventory was checked.
const RELAY_WAIT: Duration = Duration::from_secs(10);
/// The longest first line read: the token is 64 characters.
const TOKEN_LINE: u64 = 128;
/// What the agent is told of a call of Orchester's tools nobody answered.
const DENIED: &str = "Denied in Orchester.";
/// What the agent is told of a question nobody answered.
const UNANSWERED: &str = "The question was not answered: Orchester stopped waiting for it.";

/// The relays a Codex App Server invocation starts: Orchester's server for the thread, and once more for the
/// runner's one `mcpServerStatus/list`.
pub(super) const CODEX_RELAYS: usize = 2;

/// What one invocation's relay is served.
pub(super) struct Served {
  /// Claude Code's permission prompt: what it may leave to a person. `None` for Codex.
  pub(super) prompt: Option<ClaudeApprovalScope>,
  pub(super) tools: Arc<ToolHost>,
}

impl Served {
  /// How many relays show the token: Claude Code starts its permission prompt once, Codex
  /// [`CODEX_RELAYS`] times.
  pub(super) const fn relays(&self) -> usize {
    if self.prompt.is_some() { 1 } else { CODEX_RELAYS }
  }
}

/// A listener waiting for its relay, ended when this is dropped.
pub(super) struct Listening {
  /// The prompt calls a person decides.
  requests: mpsc::UnboundedReceiver<Asked>,
  /// The `ask_question` calls the person answers.
  questions: mpsc::UnboundedReceiver<AskedQuestion>,
  /// The relays that showed the token.
  taken: Arc<AtomicUsize>,
  /// Starts the last wait for the relays ([`Self::stop_accepting`]).
  stop: Option<oneshot::Sender<()>>,
  listener: JoinHandle<()>,
}

impl Drop for Listening {
  fn drop(&mut self) {
    self.listener.abort();
  }
}

/// A call the relay holds for a person.
pub(super) enum Relayed {
  /// A call a person decides.
  Approval(Asked),
  /// An `ask_question` call the person answers.
  Question(AskedQuestion),
}

impl Listening {
  /// The next call the relay holds for a person; never ready without a listener.
  pub(super) async fn next(listening: Option<&mut Self>) -> Option<Relayed> {
    let Some(listening) = listening else {
      return pending().await;
    };
    tokio::select! {
      Some(asked) = listening.requests.recv() => Some(Relayed::Approval(asked)),
      Some(asked) = listening.questions.recv() => Some(Relayed::Question(asked)),
      else => pending().await,
    }
  }

  /// Whether the relay reached the runner.
  pub(super) fn reached(&self) -> bool {
    self.taken() > 0
  }

  /// How many relays showed the token.
  pub(super) fn taken(&self) -> usize {
    self.taken.load(Ordering::Acquire)
  }

  /// Every relay the CLI starts has been started: the port closes once they have come, or after
  /// [`RELAY_WAIT`]. The relays taken are still served.
  pub(super) fn stop_accepting(&mut self) {
    if let Some(stop) = self.stop.take() {
      // The listener closes the port on its own once every relay came; then nothing waits for this.
      stop.send(()).ok();
    }
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

  /// Waits for the relays that show `token` ([`Served::relays`]), and serves them `served`.
  pub(super) fn serve(self, served: Served, token: ApprovalToken) -> Listening {
    let (sender, requests) = mpsc::unbounded_channel();
    let (asking, questions) = mpsc::unbounded_channel();
    let (stop, stopped) = oneshot::channel();
    let taken = Arc::new(AtomicUsize::new(0));
    let relay = Relay {
      token,
      served,
      requests: sender,
      questions: asking,
      taken: taken.clone(),
      asked: AtomicU64::new(0),
    };
    Listening {
      requests,
      questions,
      taken,
      stop: Some(stop),
      listener: tokio::spawn(relay.accept(self.listener, stopped)),
    }
  }
}

/// A relay's connection: its lines, and its writing half.
type Connection = (Lines<BufReader<OwnedReadHalf>>, OwnedWriteHalf);

/// What one invocation's listener serves.
struct Relay {
  token: ApprovalToken,
  served: Served,
  requests: mpsc::UnboundedSender<Asked>,
  questions: mpsc::UnboundedSender<AskedQuestion>,
  taken: Arc<AtomicUsize>,
  /// Calls of Orchester's tools asked about, and questions asked, so far, which name each such request.
  asked: AtomicU64,
}

impl Relay {
  /// Takes the relays ([`Self::take_relays`]), then the ones whose handshake was under way when the port
  /// closed, and serves them until the worker ends; their connections end with this task.
  async fn accept(self, listener: TcpListener, stop: oneshot::Receiver<()>) {
    let relay = Arc::new(self);
    let mut served = JoinSet::new();
    let mut handshakes = relay.take_relays(listener, stop, &mut served).await;
    while let Some(shown) = handshakes.join_next().await {
      relay.take(shown, &mut served);
    }
    relay.requests.closed().await;
  }

  /// Takes connections, each showing the token on its own, and serves each relay that does in `served`,
  /// until [`Served::relays`] did, [`RELAY_WAIT`] passed after `stop`, or the worker ended; the port closes
  /// when this returns, with the handshakes still under way.
  async fn take_relays(
    self: &Arc<Self>,
    listener: TcpListener,
    mut stop: oneshot::Receiver<()>,
    served: &mut JoinSet<()>,
  ) -> JoinSet<Option<Connection>> {
    let mut handshakes = JoinSet::new();
    let mut deadline: Option<Instant> = None;
    while self.taken.load(Ordering::Acquire) < self.served.relays() {
      tokio::select! {
        accepted = listener.accept(), if handshakes.len() < HANDSHAKES => match accepted {
          Ok((stream, _)) => {
            let relay = self.clone();
            handshakes.spawn(async move { relay.handshake(stream).await });
          }
          Err(error) => {
            tracing::warn!(%error, "the relay's listener cannot accept its relay");
            // Without a pause, an accept that keeps failing would spin.
            tokio::time::sleep(ACCEPT_RETRY).await;
          }
        },
        Some(shown) = handshakes.join_next() => self.take(shown, served),
        // A dropped sender is a listener that ends with the worker anyway.
        _ = &mut stop, if deadline.is_none() => deadline = Some(Instant::now() + RELAY_WAIT),
        () = sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
          tracing::warn!("the relay's listener closed before every relay of the CLI came");
          break;
        }
        () = self.requests.closed() => break,
      }
    }
    handshakes
  }

  /// Serves the relay of a finished handshake, unless it showed no token or every relay the CLI starts
  /// was taken; a handshake that panicked showed none.
  fn take(self: &Arc<Self>, shown: Result<Option<Connection>, JoinError>, served: &mut JoinSet<()>) {
    let Some((lines, write)) = shown.ok().flatten() else {
      tracing::warn!("a connection to the relay's listener did not show the invocation's token");
      return;
    };
    // Only the listener's task takes relays, one at a time.
    if self.taken.load(Ordering::Acquire) >= self.served.relays() {
      tracing::warn!("a relay came after every relay of the CLI; its connection is closed");
      return;
    }
    self.taken.fetch_add(1, Ordering::AcqRel);
    let relay = self.clone();
    served.spawn(async move { relay.serve(lines, write).await });
  }

  /// The connection's lines and its writing half once it showed the token; `None` when it did not.
  async fn handshake(&self, stream: TcpStream) -> Option<Connection> {
    let (read, write) = stream.into_split();
    let mut reader = BufReader::new(read);
    self.shows_token(&mut reader).await.then(|| (reader.lines(), write))
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
        Ok(RelayRequest::List) => Some(Value::Array(self.listed()).to_string()),
        Ok(RelayRequest::Call { name, arguments }) if self.asks_questions() && name == ask_question::NAME => self
          .ask_question(&arguments, &mut lines)
          .await
          .map(|result| result.mcp().to_string()),
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

  /// Claude Code's relay, which serves `ask_question` besides Orchester's tools; every Claude Code step has
  /// the permission prompt, so every one is served it.
  const fn asks_questions(&self) -> bool {
    self.served.prompt.is_some()
  }

  fn listed(&self) -> Vec<Value> {
    let mut tools = self.served.tools.listed();
    if self.asks_questions() {
      tools.push(ask_question::listed());
    }
    tools
  }

  /// The person's answer to an `ask_question` call with `arguments`, as its result; refused when the call
  /// is invalid; `None` when the relay closed its connection while the call waited.
  async fn ask_question(&self, arguments: &Value, lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Option<ToolResult> {
    let question = match ask_question::parse(arguments) {
      Ok(question) => question,
      Err(error) => return Some(ToolResult::unavailable(&error)),
    };
    let (reply, answered) = oneshot::channel();
    let call = format!("orchester-question-{}", self.asked.fetch_add(1, Ordering::Relaxed));
    if self.questions.send(AskedQuestion { call, question, reply }).is_err() {
      return Some(ToolResult::unavailable(UNANSWERED));
    }
    tokio::select! {
      // A dropped sender is a worker that ended without an answer.
      answered = answered => Some(answered.map_or_else(|_| ToolResult::unavailable(UNANSWERED), ToolResult::shown)),
      // The relay sends nothing while it waits; anything, or the end of the stream, ends the wait.
      _ = lines.next_line() => None,
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
