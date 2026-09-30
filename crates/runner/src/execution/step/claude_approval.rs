//! The runner's end of Claude Code's permission prompt for one invocation
//! (`orchester_core::connection::claude_approval`). The runner listens on a loopback port of its
//! own; the relay Claude Code starts (`crate::approval_relay`) connects with the invocation's
//! token and hands over each call. What the step's permissions do not leave to a person is denied
//! here at once; the rest goes to the invocation's worker, which records it and answers the call
//! with one person's decision ([`super::approval`]). The listener and every connection end with
//! the worker.

use std::{
  collections::HashSet,
  future::pending,
  io,
  net::{Ipv4Addr, SocketAddr},
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
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
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  net::{TcpListener, TcpStream},
  sync::{mpsc, oneshot},
  task::{JoinHandle, JoinSet},
  time::timeout,
};

use super::{super::run::RunTask, mcp::McpSetup};
use crate::{approval_relay::RELAY_FLAG, private_file::PrivateFile};

/// How long a connection has to show the token.
const HANDSHAKE: Duration = Duration::from_secs(10);

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
  /// Secret values of the invocation's MCP servers, kept out of what the approvals show.
  secrets: Vec<SecretVariable>,
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
    secrets: Vec<SecretVariable>,
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
    let listener = tokio::spawn(accept(listener, Arc::new(token), Arc::new(scope), sender));
    Ok(Self {
      requests,
      listener,
      config,
      secrets,
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

  pub(super) fn redact(&self, text: &str) -> String {
    redact_secrets(text, &self.secrets)
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

/// Serves each connection until the worker is gone; aborting this ends them all.
async fn accept(
  listener: TcpListener,
  token: Arc<ApprovalToken>,
  scope: Arc<ClaudeApprovalScope>,
  requests: mpsc::UnboundedSender<Asked>,
) {
  let mut connections = JoinSet::new();
  loop {
    tokio::select! {
      accepted = listener.accept() => match accepted {
        Ok((stream, _)) => {
          connections.spawn(serve(stream, token.clone(), scope.clone(), requests.clone()));
        }
        Err(error) => tracing::warn!(%error, "the permission prompt cannot accept its relay"),
      },
      Some(ended) = connections.join_next() => {
        if let Err(error) = ended {
          tracing::debug!(%error, "a permission prompt connection ended abnormally");
        }
      }
      () = requests.closed() => return,
    }
  }
}

/// One relay connection: the token first, then one call per line, each answered on one line.
async fn serve(
  stream: TcpStream,
  token: Arc<ApprovalToken>,
  scope: Arc<ClaudeApprovalScope>,
  requests: mpsc::UnboundedSender<Asked>,
) {
  let (read, mut write) = stream.into_split();
  let mut lines = BufReader::new(read).lines();
  match timeout(HANDSHAKE, lines.next_line()).await {
    Ok(Ok(Some(shown))) if token.matches(&shown) => {}
    _ => {
      tracing::warn!("a connection to the permission prompt did not show the invocation's token");
      return;
    }
  }
  while let Ok(Some(line)) = lines.next_line().await {
    let decision = match serde_json::from_str::<Value>(&line).map(|arguments| scope.classify(&arguments)) {
      Ok(ClaudeAsk::Person(request)) => ask(request, &requests).await,
      Ok(ClaudeAsk::Refused) | Err(_) => refusal(),
    };
    if write.write_all(format!("{decision}\n").as_bytes()).await.is_err() {
      return;
    }
  }
}

/// The person's decision on `request`; a denial when the worker has ended without one.
async fn ask(request: ClaudeApprovalRequest, requests: &mpsc::UnboundedSender<Asked>) -> String {
  let (reply, decision) = oneshot::channel();
  if requests.send(Asked { request, reply }).is_err() {
    return prompt_result(false);
  }
  prompt_result(decision.await.unwrap_or(false))
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
    let (names, secrets) = servers.as_ref().map_or_else(Default::default, |servers| {
      (servers.servers.clone(), servers.secrets.clone())
    });
    let Some(scope) = ClaudeApprovalScope::of(permissions, &names) else {
      return Ok(None);
    };
    let relay = self.relay.as_deref().ok_or_else(|| {
      problem("Orchester cannot find its runner program, which answers Claude Code's permission prompts".to_owned())
    })?;
    ClaudeApprovals::listen(scope, relay, secrets, worktree, (self.project.id(), invocation))
      .map(Some)
      .map_err(|error| problem(format!("cannot open the permission prompt: {error}")))
  }
}
