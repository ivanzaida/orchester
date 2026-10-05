//! The runner's end of Claude Code's permission prompt for one invocation
//! (`orchester_core::connection::claude_approval`). The relay Claude Code starts (`crate::approval_relay`)
//! reaches the runner's listener ([`super::relay`]) with the invocation's token and hands over each call.
//! What the step's permissions do not leave to a person is denied there at once; the rest goes to the
//! invocation's worker, which records it and answers the call with one person's decision
//! ([`super::approval`]). The same relay serves the step's Orchester tools ([`super::orchester_tools`]).

use std::{
  collections::HashSet,
  future::pending,
  io,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_core::{
  connection::{
    claude_approval::{
      ApprovalToken, ClaudeApprovalConfig, ClaudeApprovalRequest, ClaudeApprovalScope, not_answered, prompt_result,
    },
    stream::RunnerProblem,
  },
  id::{InvocationId, ProjectId},
  permission::AgentPermissions,
};
use tokio::sync::oneshot;

use super::{
  super::run::RunTask,
  mcp::McpSetup,
  orchester_tools::ToolHost,
  relay::{Listening, Served, bind},
};
use crate::{approval_relay::RELAY_FLAG, private_file::PrivateFile};

/// A call waiting for a person, and where the decision goes.
pub(in super::super) struct Asked {
  pub(in super::super) request: ClaudeApprovalRequest,
  pub(in super::super) reply: oneshot::Sender<PromptAnswer>,
}

/// The answer on one call: it runs, or it does not and the agent is told this and goes on.
pub(in super::super) enum PromptAnswer {
  Allow,
  Deny(String),
  /// Asked about before: denied without asking again.
  Repeated,
}

impl PromptAnswer {
  pub(super) fn result(&self) -> String {
    match self {
      Self::Allow => prompt_result(None),
      Self::Deny(told) => prompt_result(Some(told)),
      Self::Repeated => not_answered(),
    }
  }
}

/// The permission prompt of one Claude Code invocation.
pub(in super::super) struct ClaudeApprovals {
  listening: Listening,
  /// Holds the token; removed with the approvals, once the CLI has ended.
  config: PrivateFile,
  worktree: PathBuf,
  /// Calls asked about so far: each is asked once.
  asked: HashSet<String>,
}

impl ClaudeApprovals {
  /// Listens for the relay of invocation `invocation` of `project`, working in `worktree`, that
  /// leaves `scope` to a person and serves the step's `tools`, and writes the configuration file that
  /// starts `relay` to reach it.
  pub(super) fn listen(
    (scope, tools): (ClaudeApprovalScope, Arc<ToolHost>),
    relay: &Path,
    worktree: &Path,
    (project, invocation): (ProjectId, InvocationId),
  ) -> io::Result<Self> {
    let program = relay
      .to_str()
      .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the runner's path is not valid Unicode"))?;
    let token = ApprovalToken::generate();
    let served = Served {
      prompt: Some(scope.with_orchester_tools(tools.tools().clone())),
      tools,
    };
    let bound = bind()?;
    let config = ClaudeApprovalConfig::new(program, &[RELAY_FLAG], bound.address(), &token);
    let config = PrivateFile::write(project, &format!("{invocation}-approvals.json"), config.contents())?;
    let listening = bound.serve(served, token);
    Ok(Self {
      listening,
      config,
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
    match approvals.listening.requests.recv().await {
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
    self.listening.reached()
  }

  pub(super) fn worktree(&self) -> &Path {
    &self.worktree
  }
}

impl RunTask {
  /// The permission prompt of Claude Code invocation `invocation`, working in `worktree` with
  /// `permissions`, the MCP servers of `setup` and the Orchester tools of `tools`; `None` for Codex, or
  /// when the step asks a person for nothing (and so gets no Orchester tools either).
  pub(super) fn claude_approvals(
    &self,
    (permissions, setup, tools): (&AgentPermissions, &McpSetup, &Arc<ToolHost>),
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
    let scope = scope.with_secrets(secrets);
    let relay = self.relay.as_deref().ok_or_else(|| {
      problem("Orchester cannot find its runner program, which answers Claude Code's permission prompts".to_owned())
    })?;
    ClaudeApprovals::listen((scope, tools.clone()), relay, worktree, (self.project.id(), invocation))
      .map(Some)
      .map_err(|error| problem(format!("cannot open the permission prompt: {error}")))
  }
}
