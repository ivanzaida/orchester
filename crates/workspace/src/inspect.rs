//! Empty `DevtoolsInspectable` markers behind the modern and legacy `devtools-inspect` features.
//! feature. lurq requires the trait of component props and, with its DevTools, of signal values; an
//! app cannot implement it for these types itself (orphan rule). This is the only place the crate
//! uses lurq.

macro_rules! markers {
  ($inspect:path) => {
    impl $inspect for crate::accounts::AccountUses {}
    impl $inspect for crate::accounts::NewAccount {}
    impl $inspect for crate::tasks::plan_limit::PlanStop {}
    impl $inspect for crate::activity::Activity {}
    impl $inspect for crate::activity::ActivityLog {}
    impl $inspect for crate::activity::OperationSpec {}
    impl $inspect for crate::agents::data::ProfileChange {}
    impl $inspect for crate::agents::data::ProjectAgents {}
    impl $inspect for crate::agents::form::ModelSelection {}
    impl $inspect for crate::clock::Clock {}
    impl $inspect for crate::close::CloseFlow {}
    impl $inspect for crate::close::ClosePhase {}
    impl $inspect for crate::close::WaitingTask {}
    impl $inspect for crate::code_index::IndexList {}
    impl $inspect for crate::code_index::RipgrepCheck {}
    impl $inspect for crate::connections::data::ConnectionDraft {}
    impl $inspect for crate::connections::data::ProjectConnections {}
    impl $inspect for crate::connections::github::GitHubDraft {}
    impl $inspect for crate::connections::mcp::TransportChoice {}
    impl $inspect for crate::connections::native::NativeDraft {}
    impl $inspect for crate::import::data::Catalogs {}
    impl $inspect for crate::import::data::Sources {}
    impl $inspect for crate::instructions::data::NewInstruction {}
    impl $inspect for crate::instructions::data::ProjectInstructions {}
    impl $inspect for crate::instructions::form::Events {}
    impl<T> $inspect for crate::instructions::form::Picked<T> {}
    impl $inspect for crate::instructions::form::ScopeChoice {}
    impl $inspect for crate::instructions::form::Target {}
    impl $inspect for crate::instructions::preview::PreviewCatalog {}
    impl $inspect for crate::instructions::preview::PreviewChoice {}
    impl $inspect for crate::messages::Message {}
    impl $inspect for crate::orchestrator::chat::ChatView {}
    impl $inspect for crate::orchestrator::chat::History {}
    impl $inspect for crate::orchestrator::send::Outgoing {}
    impl $inspect for crate::orchestrator::send::SendError {}
    impl $inspect for crate::orchestrator::settings::OrchestratorSetup {}
    impl $inspect for crate::orchestrator::settings::Saving {}
    impl $inspect for crate::pipelines::data::OpenedPipeline {}
    impl $inspect for crate::pipelines::fields::AccessChoice {}
    impl $inspect for crate::pipelines::fields::Executor {}
    impl $inspect for crate::pipelines::fields::ModelChoice {}
    impl $inspect for crate::pipelines::fields::RouteChoice {}
    impl $inspect for crate::pipelines::history::VersionHistory {}
    impl $inspect for crate::pipelines::library::LibraryEntry {}
    impl $inspect for crate::pipelines::library::StartFrom {}
    impl $inspect for crate::pipelines::problems::ProblemView {}
    impl $inspect for crate::plan_limits::Connections {}
    impl $inspect for crate::plan_limits::Readings {}
    impl $inspect for crate::project_list::Listing {}
    impl $inspect for crate::project_setup::CreateFailure {}
    impl $inspect for crate::project_setup::Creation {}
    impl $inspect for crate::pull_requests::Inbox {}
    impl $inspect for crate::pull_requests::detail::Detail {}
    impl $inspect for crate::pull_requests::list::PullRequestList {}
    impl $inspect for crate::pull_requests::list::PullRequestSort {}
    impl $inspect for crate::pull_requests::page::PullRequestPage {}
    impl $inspect for crate::recovery::InterruptedRun {}
    impl $inspect for crate::recovery::Recovery {}
    impl $inspect for crate::recovery::RecoveryState {}
    impl $inspect for crate::repository::DetectTarget {}
    impl $inspect for crate::runner::ActiveWork {}
    impl $inspect for crate::runner::RunnerStatus {}
    impl $inspect for crate::runner::Runners {}
    impl $inspect for crate::runner::SaveState {}
    impl $inspect for crate::runner::state::RunnerState {}
    impl $inspect for crate::secrets::Vault {}
    impl $inspect for crate::secrets::data::ProjectSecrets {}
    impl $inspect for crate::sharing::PendingPasswordChange {}
    impl $inspect for crate::sharing::Servers {}
    impl $inspect for crate::sharing::SignedIn {}
    impl $inspect for crate::sharing::project::IdentityProblem {}
    impl $inspect for crate::sharing::project::JoinReview {}
    impl $inspect for crate::sharing::project::Joined {}
    impl $inspect for crate::sharing::project::MemberChange {}
    impl $inspect for crate::sharing::project::SharePlan {}
    impl $inspect for crate::sharing::project::Shared {}
    impl $inspect for crate::sharing::project::discovery::Discovery {}
    impl $inspect for crate::sharing::project::discovery::DiscoveryFile {}
    impl $inspect for crate::sharing::project::lookup::Lookup {}
    impl $inspect for crate::sharing::project::state::ProjectSharing {}
    impl $inspect for crate::skills::data::ProjectSkills {}
    impl $inspect for crate::skills::repository::RepositoryScan {}
    impl $inspect for crate::skills::editor::EditorData {}
    impl $inspect for crate::tasks::browse::PipelineFilter {}
    impl $inspect for crate::tasks::browse::Selection {}
    impl $inspect for crate::tasks::browse::TaskSort {}
    impl $inspect for crate::tasks::cancel_run::Stop {}
    impl $inspect for crate::tasks::changes::Checkpointed {}
    impl $inspect for crate::tasks::changes::NotCheckpointed {}
    impl $inspect for crate::tasks::conflict::EditConflict {}
    impl $inspect for crate::tasks::conversation::Conversation {}
    impl $inspect for crate::tasks::conversation::follow::Sent {}
    impl $inspect for crate::tasks::conversation::outgoing::Context {}
    impl $inspect for crate::tasks::data::ProjectTasks {}
    impl $inspect for crate::tasks::detail::TaskDetail {}
    impl $inspect for crate::tasks::integration::IntegrationProblem {}
    impl $inspect for crate::tasks::integration::LeftBehind {}
    impl $inspect for crate::tasks::integration::Merged {}
    impl $inspect for crate::tasks::integration_jobs::JobState {}
    impl $inspect for crate::tasks::integration_jobs::ProjectJobState {}
    impl $inspect for crate::tasks::integration_jobs::RunJobState {}
    impl $inspect for crate::tasks::list::TaskRow {}
    impl $inspect for crate::tasks::list::TaskTab {}
    impl $inspect for crate::tasks::outside::OutsideChanges {}
    impl $inspect for crate::tasks::outside::OutsideCounts {}
    impl $inspect for crate::tasks::overview::OverviewTab {}
    impl $inspect for crate::tasks::plan::PlanReview {}
    impl $inspect for crate::tasks::ready_to_run::ProjectReadiness {}
    impl $inspect for crate::tasks::question::Answering {}
    impl $inspect for crate::tasks::question::Draft {}
    impl $inspect for crate::tasks::run_again::RunAgainCheck {}
    impl $inspect for crate::tasks::run_approvals::AutoModeState {}
    impl $inspect for crate::tasks::run_approvals::RunApprovalChange {}
    impl $inspect for crate::tasks::timeline::load::Loaded {}
    impl $inspect for crate::tasks::timeline::load::Requests {}
    impl $inspect for crate::tasks::usage::ProjectUsage {}
    impl $inspect for crate::tools::data::ProjectTools {}
    impl $inspect for crate::tools::data::ToolChange {}
    impl $inspect for crate::workspace::ListedData {}
    impl $inspect for crate::workspace::OpenProject {}
    impl $inspect for crate::workspace::ProjectData {}
    impl $inspect for crate::workspace::ProjectEntry {}
    impl $inspect for crate::workspace::Workspace {}
  };
}

#[cfg(feature = "devtools-inspect")]
markers!(lurq::app::component::DevtoolsInspectable);
#[cfg(feature = "devtools-inspect-legacy")]
markers!(lurq_legacy::app::component::DevtoolsInspectable);
