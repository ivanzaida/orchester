//! The tools of Orchester's own MCP server (`docs/stable-build.md`, "Orchester's MCP server"):
//! automation, such as the coordinator agent that develops Orchester through Orchester, controls
//! projects, tasks, runs, approvals and pipelines through data-level tools instead of clicking the
//! window. Each tool goes through the services the screens use ([`Services`]), so it follows the same
//! rules, writes the same records and raises the same events. The server around them (its transport,
//! guard and discovery file) is each app's.

mod error;
mod protocol;
mod services;
mod tools;

pub use self::{
  error::{Missing, ToolError},
  protocol::supported_protocol_versions,
  services::Services,
  tools::{Proposal, Translate, call, find_run, listed, propose_run, run_proposed, setup},
};
pub(crate) use self::{
  services::{PersonDecision, blocking},
  tools::{
    ContinueChoice, ContinueRun, SendTiming, answer_for_orchestrator, check_answer, continue_for_orchestrator,
    continue_offered, duplicate_pipeline_draft, find_task, not_in_group, outgoing_refusal, plan_person_only, sent_as,
    waiting_request,
  },
};
