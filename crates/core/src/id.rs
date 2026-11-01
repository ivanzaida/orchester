use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Defines a typed UUIDv7 identifier so ids of different entities cannot be swapped.
macro_rules! define_id {
  ($(#[$doc:meta])* $name:ident) => {
    $(#[$doc])*
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct $name(Uuid);

    impl $name {
      /// A new time-ordered id.
      pub fn generate() -> Self {
        Self(Uuid::now_v7())
      }

      pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
      }

      pub const fn as_uuid(self) -> Uuid {
        self.0
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
      }
    }

    impl FromStr for $name {
      type Err = uuid::Error;

      fn from_str(value: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(value).map(Self)
      }
    }
  };
}

define_id!(
  /// An account on a shared server.
  UserId
);
define_id!(
  /// A revocable sign-in on a shared server.
  SessionId
);
define_id!(
  /// An invite a team server made: to the server, to one of its projects, or for an existing account.
  InviteId
);
define_id!(
  /// A cloud join of an existing account that waits for that account's devices or an admin (join approval).
  ApprovalId
);
define_id!(
  /// A team server's own id, made on its first start: the audience of Orchester Cloud's identity tokens for it.
  TeamServerId
);
define_id!(
  /// One installation of the app, i.e. one application database (`orchester.db`): what keeps the secrets of two
  /// installations on one computer, e.g. a development build's and the installed app's, apart.
  InstallationId
);
define_id!(
  /// A shared server in the desktop's list, given by the desktop when the server is added.
  ServerId
);
define_id!(
  /// A project, i.e. one repository managed by Orchester.
  ProjectId
);
define_id!(
  /// An agent CLI account on this computer (`docs/design/agent-accounts.md`): a folder where the CLI keeps its own
  /// sign-in, shared by every project.
  AgentAccountId
);
define_id!(
  /// A versioned configuration resource (agent profile, instruction, pipeline, tool).
  ResourceId
);
define_id!(
  /// One immutable published version of a resource.
  VersionId
);
define_id!(
  /// An account on Orchester Cloud, the identity service (`docs/architecture/server.md`, "Orchester Cloud").
  CloudAccountId
);
define_id!(
  /// One signed-in device of a cloud account: a desktop app or a web browser.
  CloudDeviceId
);
define_id!(TaskId);
define_id!(TagId);
define_id!(
  /// A module of a project's tasks (`crate::module`): an epic, a milestone or a sprint.
  ModuleId
);
define_id!(
  /// One tenure of a shared task's holder.
  ClaimId
);
define_id!(
  /// One execution of a task; names its branch and worktree.
  RunId
);
define_id!(
  /// A project connection to a repository host, agent provider or MCP server.
  ConnectionId
);
define_id!(
  /// A project's connection to a decision model's server (`crate::connection::DecisionConnection`). Its own type,
  /// so no agent profile, which names a [`ConnectionId`], can choose one.
  DecisionConnectionId
);
define_id!(
  /// A project secret. Bodies and rows hold only this reference; the value stays in the OS keychain.
  SecretId
);
define_id!(
  /// Client-generated key that makes a create/queue/send action safe to retry.
  IdempotencyKey
);

impl TaskId {
  /// Reopening a queued task must retry its first run with the same key. Later runs use
  /// independent request keys.
  pub const fn first_run_key(self) -> IdempotencyKey {
    IdempotencyKey::from_uuid(self.as_uuid())
  }
}
define_id!(
  /// One attempt of a run; a failure routed back to an upstream step starts a new attempt.
  AttemptId
);
define_id!(
  /// One execution of a pipeline step within an attempt.
  StepExecutionId
);
define_id!(
  /// A persisted point a run can continue from after a pause or an interruption.
  CheckpointId
);
define_id!(
  /// One launch of an agent CLI for a step execution: the first, or a retry or continue.
  InvocationId
);
define_id!(
  /// One message of a task conversation, a person's or an agent's.
  MessageId
);
define_id!(
  /// One record of task or project memory.
  MemoryId
);
define_id!(
  /// One notice to a run that its base branch moved (`crate::base_move`).
  NoticeId
);
define_id!(
  /// One native agent conversation: the items of one step execution, shared by its invocations.
  ConversationId
);
define_id!(
  /// One conversation of a project's Orchestrator chat (`crate::orchestrator`).
  ChatId
);
define_id!(
  /// One message a person wrote in the Orchestrator chat, given when it is first sent so a retry stores it once.
  ChatMessageId
);
define_id!(
  /// One file a person attached to a message (`crate::attachment`).
  AttachmentId
);
define_id!(
  /// One thread of agent feedback on a pull request (`crate::pull_request::feedback`).
  FeedbackId
);
define_id!(
  /// One request or reply a person wrote in a feedback thread.
  FeedbackPostId
);
define_id!(
  /// One document of a project's Docs (`crate::docs`).
  DocId
);
define_id!(
  /// One folder of a project's Docs tree (`crate::docs`).
  DocFolderId
);
