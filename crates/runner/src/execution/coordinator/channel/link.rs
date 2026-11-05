//! The coordinator link over the channel: each request crosses as JSON to the coordinator's side, which answers it with
//! the link it holds (the local one), and the answer, a failure included, crosses back as JSON.

use std::sync::Arc;

use orchester_core::{
  agent_files::RepositoryAgentFiles,
  base_move::BaseNotice,
  code_index::CodeIndexSetting,
  conversation::Message,
  memory::{MemorySettings, Recalled},
  question::AskedQuestions,
  resource::CommandEnvironment,
  run::{
    ApprovalRule, EffectiveMode, PendingToolApproval, ProjectRule,
    review::{AutoModeSettings, Review},
  },
};

use super::{Calls, Reply, Wire, crossed_whole, received, whole};
use crate::execution::executor::link::{
  ApprovalLink, BaseNoticeLink, CoordinatorLink, EnvironmentLink, LinkError, MemoryLink, MessageLink, QuestionLink,
  ResourceLink, SettingsLink, UsageLink,
  approvals::{ApprovalRequest, AutoApproveRequest, PendingApprovalsRequest, ProjectRulesRequest, RunRulesRequest},
  base_notices::{PendingNoticeRequest, TellsBaseMovesRequest},
  environment::EnvironmentRequest,
  memory::{RecallRequest, SettingsRequest, UnreadableMemories, UnreadableRequest},
  messages::{AttachedContent, AttachedRequest, MessagesRequest},
  questions::{QuestionRequestById, QuestionsRequest},
  resources::{Blob, BlobRequest, PinnedBody, ResourceNames, ResourceRequest, VersionRequest},
  settings::{
    AgentFilesRequest, ApprovalModeRequest, AutoModeRequest, CodeIndexRequest, QaAppsAtOnce, QaAppsRequest,
    WarmFolders, WarmFoldersRequest,
  },
  usage::{StepUsage, StepUsageRequest},
};

type Backend = Box<dyn CoordinatorLink + Send>;

/// The executor's end of a coordinator link whose other end, on the coordinator's side, is `Backend`.
pub(in crate::execution) struct ChannelLink {
  wire: Wire<Backend>,
}

impl ChannelLink {
  /// A link whose requests `link` answers on the coordinator's side; each crossing is counted in `calls`.
  pub(in crate::execution) fn over(link: Backend, calls: Arc<Calls>) -> Self {
    Self {
      wire: Wire::open(link, serve, calls),
    }
  }
}

/// Answers the request `text` holds for `method` with `link`'s method of that name.
fn serve(link: &mut Backend, method: &str, text: &str) -> Reply {
  macro_rules! answer {
    ($($name:ident),* $(,)?) => {
      match method {
        $(stringify!($name) => whole(&link.$name(&received(text))),)*
        other => panic!("the coordinator link has no request `{other}`"),
      }
    };
  }
  answer!(
    resource,
    version,
    blob,
    messages,
    attached,
    approval_rules,
    project_approval_rules,
    run_auto_approve,
    pending_approvals,
    approval_answered,
    approval_review,
    questions,
    question,
    project_environment,
    recall,
    unreadable,
    settings,
    code_index,
    agent_files,
    approval_mode,
    auto_mode,
    warm_folders,
    qa_apps_at_once,
    step_usage,
    tells_base_moves,
    pending_notice,
  )
}

/// Methods that send their request over the channel and give the answer read back.
macro_rules! asked {
  ($($name:ident($request:ty) -> $answer:ty;)*) => {
    $(
      fn $name(&self, request: &$request) -> Result<$answer, LinkError> {
        crossed_whole(self.wire.ask(stringify!($name), request))
      }
    )*
  };
}

impl ResourceLink for ChannelLink {
  asked! {
    resource(ResourceRequest) -> Option<ResourceNames>;
    version(VersionRequest) -> Option<PinnedBody>;
    blob(BlobRequest) -> Blob;
  }
}

impl MessageLink for ChannelLink {
  asked! {
    messages(MessagesRequest) -> Vec<Message>;
    attached(AttachedRequest) -> Vec<AttachedContent>;
  }
}

impl ApprovalLink for ChannelLink {
  asked! {
    approval_rules(RunRulesRequest) -> Vec<ApprovalRule>;
    project_approval_rules(ProjectRulesRequest) -> Vec<ProjectRule>;
    run_auto_approve(AutoApproveRequest) -> Option<bool>;
    pending_approvals(PendingApprovalsRequest) -> Vec<PendingToolApproval>;
    approval_answered(ApprovalRequest) -> bool;
    approval_review(ApprovalRequest) -> Option<Review>;
  }
}

impl QuestionLink for ChannelLink {
  asked! {
    questions(QuestionsRequest) -> Vec<AskedQuestions>;
    question(QuestionRequestById) -> Option<AskedQuestions>;
  }
}

impl EnvironmentLink for ChannelLink {
  asked! {
    project_environment(EnvironmentRequest) -> CommandEnvironment;
  }
}

impl MemoryLink for ChannelLink {
  asked! {
    recall(RecallRequest) -> Recalled;
    unreadable(UnreadableRequest) -> UnreadableMemories;
    settings(SettingsRequest) -> MemorySettings;
  }
}

impl SettingsLink for ChannelLink {
  asked! {
    code_index(CodeIndexRequest) -> CodeIndexSetting;
    agent_files(AgentFilesRequest) -> RepositoryAgentFiles;
    approval_mode(ApprovalModeRequest) -> EffectiveMode;
    auto_mode(AutoModeRequest) -> AutoModeSettings;
    warm_folders(WarmFoldersRequest) -> WarmFolders;
    qa_apps_at_once(QaAppsRequest) -> QaAppsAtOnce;
  }
}

impl UsageLink for ChannelLink {
  asked! {
    step_usage(StepUsageRequest) -> StepUsage;
  }
}

impl BaseNoticeLink for ChannelLink {
  asked! {
    tells_base_moves(TellsBaseMovesRequest) -> bool;
    pending_notice(PendingNoticeRequest) -> Option<BaseNotice>;
  }
}

impl CoordinatorLink for ChannelLink {
  /// Another link over a channel of its own, to the link the coordinator's side opened beside its own.
  fn another(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError> {
    let opened = self.wire.on_coordinator_side(|link: &mut Backend| link.another())?;
    Ok(Box::new(Self::over(opened, self.wire.counted.clone())))
  }
}
