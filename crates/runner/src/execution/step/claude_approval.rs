//! The runner's end of Claude Code's permission prompt for one invocation
//! (`orchester_core::connection::claude_approval`). The runner listens on a loopback port of its
//! own; the relay Claude Code starts (`crate::approval_relay`) connects with the invocation's
//! token and hands over each call. What the step's permissions do not leave to a person is denied
//! here at once; the rest goes to the invocation's worker, which records it and answers the call
//! with one person's decision ([`super::approval`]). The listener and its connection end with
//! the worker.
//!
//! Only one relay is served: connections are taken one at a time, each must show the token on a
//! short first line within [`HANDSHAKE`] (compared in constant time), and after the first that
//! does, or after [`ATTEMPTS`] that do not, nothing more is accepted. While a call waits, the
//! connection is watched: a relay that goes away ends the wait, so a later decision cannot be
//! recorded as delivered.

use std::{
  collections::HashSet,
  future::pending,
  io,
  net::{Ipv4Addr, SocketAddr},
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  connection::{
    claude_approval::{
      ApprovalToken, ClaudeApprovalConfig, ClaudeApprovalRequest, ClaudeApprovalScope, ClaudeAsk, prompt_result,
      refusal,
    },
    stream::RunnerProblem,
  },
  id::{InvocationId, ProjectId},
  permission::AgentPermissions,
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

use super::{super::run::RunTask, mcp::McpSetup};
use crate::{approval_relay::RELAY_FLAG, private_file::PrivateFile};

/// How long a connection has to show the token.
const HANDSHAKE: Duration = Duration::from_secs(10);
/// How long the listener waits after a failed accept before the next.
const ACCEPT_RETRY: Duration = Duration::from_millis(100);
/// Connections without the token before the listener gives up.
const ATTEMPTS: usize = 8;
/// The longest first line read: the token is 64 characters.
const TOKEN_LINE: u64 = 128;

/// A call waiting for a person, and where the decision goes.
pub(in super::super) struct Asked {
  pub(in super::super) request: ClaudeApprovalRequest,
  pub(in super::super) reply: oneshot::Sender<bool>,
}

/// The permission prompt of one Claude Code invocation.
pub(in super::super) struct ClaudeApprovals {
  requests: mpsc::UnboundedReceiver<Asked>,
  listener: JoinHandle<()>,
  /// Holds the token; removed with the approvals, once the CLI has ended.
  config: PrivateFile,
  /// Set once the relay showed the token.
  reached: Arc<AtomicBool>,
  worktree: PathBuf,
  /// Calls asked about so far: each is asked once.
  asked: HashSet<String>,
}

impl ClaudeApprovals {
  /// Listens for the relay of invocation `invocation` of `project`, working in `worktree`, that
  /// leaves `scope` to a person, and writes the configuration file that starts `relay` to reach it.
  pub(super) fn listen(
    scope: ClaudeApprovalScope,
    relay: &Path,
    worktree: &Path,
    (project, invocation): (ProjectId, InvocationId),
  ) -> io::Result<Self> {
    let program = relay
      .to_str()
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the runner's path is not valid Unicode"))?;
    let listener = std::net::TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?.to_string();
    let listener = TcpListener::from_std(listener)?;
    let token = ApprovalToken::generate();
    let config = ClaudeApprovalConfig::new(program, &[RELAY_FLAG], &address, &token);
    let config = PrivateFile::write(project, &format!("{invocation}-approvals.json"), config.contents())?;
    let (sender, requests) = mpsc::unbounded_channel();
    let reached = Arc::new(AtomicBool::new(false));
    let relay = Relay {
      token,
      scope,
      requests: sender,
      reached: reached.clone(),
    };
    let listener = tokio::spawn(relay.accept(listener));
    Ok(Self {
      requests,
      listener,
      config,
      reached,
      worktree: worktree.to_path_buf(),
      asked: HashSet::new(),
    })
  }

  /// The configuration file Claude Code starts the relay from.
  pub(super) fn config(&self) -> &Path {
    self.config.path()
  }

  /// The next call a person decides; never ready without approvals.
  pub(super) async fn next(approvals: Option<&mut Self>) -> Option<Asked> {
    let Some(approvals) = approvals else {
      return pending().await;
    };
    match approvals.requests.recv().await {
      Some(asked) => Some(asked),
      None => pending().await,
    }
  }

  /// Records that `call` is asked about; `false` when it was before.
  pub(super) fn first_ask(&mut self, call: &str) -> bool {
    self.asked.insert(call.to_owned())
  }

  /// Whether the relay reached the runner: without it, Claude Code has no permission prompt.
  pub(super) fn reached(&self) -> bool {
    self.reached.load(Ordering::Acquire)
  }

  pub(super) fn worktree(&self) -> &Path {
    &self.worktree
  }
}

impl Drop for ClaudeApprovals {
  fn drop(&mut self) {
    self.listener.abort();
  }
}

/// What one invocation's listener serves.
struct Relay {
  token: ApprovalToken,
  scope: ClaudeApprovalScope,
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
            tracing::warn!(%error, "the permission prompt cannot accept its relay");
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
        tracing::warn!("a connection to the permission prompt did not show the invocation's token");
        continue;
      }
      self.reached.store(true, Ordering::Release);
      // No other connection is taken while or after this one is served.
      drop(listener);
      return self.serve(reader.lines(), write).await;
    }
    tracing::warn!("the permission prompt stopped listening after {ATTEMPTS} connections without its token");
  }

  async fn shows_token(&self, reader: &mut BufReader<OwnedReadHalf>) -> bool {
    let mut line = String::new();
    let read = timeout(HANDSHAKE, reader.take(TOKEN_LINE).read_line(&mut line)).await;
    matches!(read, Ok(Ok(length)) if length > 0) && self.token.matches(line.trim_end())
  }

  /// One call per line, each answered on one line.
  async fn serve(&self, mut lines: Lines<BufReader<OwnedReadHalf>>, mut write: OwnedWriteHalf) {
    while let Ok(Some(line)) = lines.next_line().await {
      let decision = match serde_json::from_str::<Value>(&line).map(|arguments| self.scope.classify(&arguments)) {
        Ok(ClaudeAsk::Person(request)) => match self.ask(request, &mut lines).await {
          Some(decision) => decision,
          // The relay went away while the call waited: the worker's wait ends with it.
          None => return,
        },
        Ok(ClaudeAsk::Refused) | Err(_) => refusal(),
      };
      if write.write_all(format!("{decision}\n").as_bytes()).await.is_err() {
        return;
      }
    }
  }

  /// The person's decision on `request`; a denial when the worker ended without one; `None` when
  /// the relay closed its connection or broke the protocol while the call waited.
  async fn ask(&self, request: ClaudeApprovalRequest, lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Option<String> {
    let (reply, decision) = oneshot::channel();
    if self.requests.send(Asked { request, reply }).is_err() {
      return Some(prompt_result(false));
    }
    tokio::select! {
      // A dropped sender is a worker that ended without a decision: the call is denied.
      decision = decision => Some(prompt_result(decision.unwrap_or(false))),
      // The relay sends nothing while it waits; anything, or the end of the stream, ends the wait.
      _ = lines.next_line() => None,
    }
  }
}

impl RunTask {
  /// The permission prompt of Claude Code invocation `invocation`, working in `worktree` with
  /// `permissions` and the MCP servers of `setup`; `None` for Codex, or when the step asks a
  /// person for nothing.
  pub(super) fn claude_approvals(
    &self,
    permissions: &AgentPermissions,
    setup: &McpSetup,
    (invocation, worktree): (InvocationId, &Path),
  ) -> Result<Option<ClaudeApprovals>, RunnerProblem> {
    let McpSetup::Claude(servers) = setup else {
      return Ok(None);
    };
    let problem = |detail: String| RunnerProblem::CouldNotStart { detail };
    let names = servers
      .as_ref()
      .map(|servers| servers.servers.clone())
      .unwrap_or_default();
    let Some(scope) = ClaudeApprovalScope::of(permissions, &names) else {
      return Ok(None);
    };
    let relay = self.relay.as_deref().ok_or_else(|| {
      problem("Orchester cannot find its runner program, which answers Claude Code's permission prompts".to_owned())
    })?;
    ClaudeApprovals::listen(scope, relay, worktree, (self.project.id(), invocation))
      .map(Some)
      .map_err(|error| problem(format!("cannot open the permission prompt: {error}")))
  }
}
