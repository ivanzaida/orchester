//! `apply_proposal`, a tool of the Orchestrator chat's own (ORC-96, `docs/design/orchestrator-toolset.md`, "Approving
//! in the chat"): the person may approve a proposal of the chat in their own words ("run it", "apply", "так")
//! instead of clicking its card, and the chat then applies it for them. Only in the turn the person's own message
//! started (not one a wait, a run's end or a day's report started by itself, and not a message automation sent through
//! the window), only a proposal of this conversation that waited when the person wrote, and never one only a click
//! applies ([`Change::click_only`], or a publication that widens what agents may do). It is applied exactly as the
//! card's *Apply* applies it ([`super::proposal::accept`]): checked again, refused once what it stands on changed, and
//! recorded as the person's decision with the message that approved it, which the card names. Which proposal the
//! message approves, and whether it changes what was proposed, is the chat's to judge by its instructions: with
//! several pending and no word of which, it asks; a message that also changes the proposal makes a new one.

use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction, ChatApproval, ChatEntry, EntryKind,
    change::{ClickOnly, ProposedChange},
    chat::title_of,
  },
  run::Answerer,
};
use orchester_store::ProjectDb;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{change, own_project::in_own_project, proposal, tools::ChatScope};
use crate::{
  app_mcp::{Services, ToolError, Translate, blocking},
  messages::Message,
  runner::Runners,
};

/// The tool's name.
pub const APPLY_PROPOSAL: &str = "apply_proposal";

/// What the chat's agent reads of the tool.
const DESCRIPTION: &str = "Applies a proposal of yours that waits for the person in this conversation (a card: a run's \
  start, a publication, a cancel, a merge, an answer, a message, a pause, a resume, a release, how a stopped run goes \
  on, a first commit, a narrowing of an MCP server), for the person, when their message in this turn clearly approves \
  it, such as \"run it\", \"apply it\", \"yes, do it\", \"так\" or \"застосовуй\". It is applied exactly as the card's \
  Apply applies it, as the person's own decision, and the card says it was applied by them in the chat. Without \
  proposal it lists the proposals that wait (each with its number, what it does and whether only a click applies it) \
  and applies nothing; then call it again with the number the person's message approves. When several wait and the \
  message does not say which, ask the person which one; never guess. When the message also changes the proposal (\"run \
  it, but without the tests\"), do not apply it: make a new proposal instead. Only in a turn the person's own message \
  started: refused in a turn Orchester started by itself (not_persons_turn) and for a message automation sent \
  (automation_input). Refused for a proposal made after the person's message or that no longer waits (not_pending), and \
  for what only a click on its card applies (click_only): anything that widens what agents may do (adding or widening \
  an MCP server, a publication that widens), git's work that can lose work (git_propose), and a first commit that adds \
  files that look as if they hold secrets; tell the person to click Apply on the card then. Refused as the card's \
  Apply refuses it, e.g. changed_since_proposed once what it stands on changed: say so, and propose it again if the \
  person still wants it.";

/// The tool as the chat's `tools/list` answers.
pub fn tool() -> Tool {
  Tool::new(APPLY_PROPOSAL, DESCRIPTION, schema_for_type::<ApplyProposal>())
    .with_annotations(ToolAnnotations::new().read_only(false).open_world(false))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ApplyProposal {
  /// The project's id, or its name: the chat's own.
  #[serde(default)]
  pub project: Option<String>,
  /// The number of the proposal to apply, as this tool lists it. Left out, the proposals that wait are listed and
  /// nothing is applied.
  #[serde(default)]
  pub proposal: Option<i64>,
}

/// Applies the proposal `arguments` name in `scope` for the person who approved it in the turn's message, or lists the
/// proposals that wait; `runners` tell the chat its card changed.
pub(super) async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  in_own_project(scope, &mut arguments)?;
  let asked: ApplyProposal =
    serde_json::from_value(Value::Object(arguments)).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
  if scope.on_its_own {
    return Err(refused("not_persons_turn", "orchestrator.approval.notPersonsTurn"));
  }
  let (root, chat) = (scope.root.clone(), scope.chat);
  let entries = blocking(move || Ok(ProjectDb::open(&root)?.chat_entries(chat)?)).await?;
  let approval = approver(&entries)?;
  let pending = pending(&entries, &approval);
  let Some(seq) = asked.proposal else {
    return Ok(json!({ "applied": false, "pending": listed(translate, scope, &pending).await? }));
  };
  let Some((_, action)) = pending.iter().find(|(at, _)| *at == seq) else {
    return Err(ToolError::refused(
      "not_pending",
      vec![Message::new("orchestrator.approval.notPending").with("proposal", seq.to_string())],
    ));
  };
  if let Some(why) = click_only(scope, action.change.as_deref()).await? {
    return Err(ToolError::refused("click_only", vec![click_only_reason(why)]));
  }
  let carried = proposal::accept((services, translate, runners), scope, seq, (&[], Some(approval)))
    .await
    .map_err(|message| ToolError::refused("not_pending", vec![message]))??;
  Ok(json!({
    "applied": true,
    "proposal": seq,
    "result": carried,
    "message": translate.text(&Message::new("orchestrator.approval.applied")),
  }))
}

/// The person's message that started the turn under way among a conversation's `entries`: refused when no person's
/// message did (a wait, a run's end or a day's report started it by itself) or automation sent it.
fn approver(entries: &[ChatEntry]) -> Result<ChatApproval, ToolError> {
  let thinking = entries
    .iter()
    .rev()
    .find(|entry| entry.kind.turn().is_some_and(|(_, state)| state.is_thinking()));
  match thinking.map(|entry| (entry.seq, &entry.kind)) {
    Some((
      seq,
      EntryKind::Person {
        message,
        text,
        sent_by: Answerer::Person,
        ..
      },
    )) => Ok(ChatApproval {
      message: *message,
      seq,
      words: title_of(text.as_str()),
    }),
    Some((_, EntryKind::Person { .. })) => Err(refused("automation_input", "orchestrator.approval.automationInput")),
    _ => Err(refused("not_persons_turn", "orchestrator.approval.notPersonsTurn")),
  }
}

/// The proposals among `entries` that wait for the person and were there when they wrote `approval`'s message: one
/// proposed after it was never shown to them.
fn pending(entries: &[ChatEntry], approval: &ChatApproval) -> Vec<(i64, ChatAction)> {
  entries
    .iter()
    .take_while(|entry| entry.seq < approval.seq)
    .filter_map(|entry| match &entry.kind {
      EntryKind::Action(action) if action.waits() => Some((entry.seq, action.clone())),
      _ => None,
    })
    .collect()
}

/// Why only a click on the card applies `change` in `scope`; `None` when the person may approve it in the chat. A
/// start is never click-only.
async fn click_only(scope: &ChatScope, change: Option<&ProposedChange>) -> Result<Option<ClickOnly>, ToolError> {
  let Some(change) = change else {
    return Ok(None);
  };
  if let Some(why) = change.change.click_only() {
    return Ok(Some(why));
  }
  let widens = change::widens(scope, change).await?;
  Ok((!widens.is_empty()).then_some(ClickOnly::WidensPermissions))
}

fn click_only_reason(why: ClickOnly) -> Message {
  Message::new(match why {
    ClickOnly::WidensPermissions => "orchestrator.approval.clickOnly.widens",
    ClickOnly::LosesWork => "orchestrator.approval.clickOnly.losesWork",
    ClickOnly::LooksSecret => "orchestrator.approval.clickOnly.looksSecret",
  })
}

/// The proposals that wait, as the agent reads them: each with its number, what it proposes and whether only a click
/// applies it.
async fn listed(
  translate: &Translate,
  scope: &ChatScope,
  pending: &[(i64, ChatAction)],
) -> Result<Vec<Value>, ToolError> {
  let mut listed = Vec::with_capacity(pending.len());
  for (seq, action) in pending {
    let proposed = match (&action.outcome, action.change.as_deref()) {
      (ActionOutcome::Proposed(start), _) => json!({ "start": { "task": action.task, "pipeline": start.pipeline } }),
      (_, Some(change)) => change::told(translate, change)["proposed"].clone(),
      (_, None) => Value::Null,
    };
    let click_only = click_only(scope, action.change.as_deref()).await?.is_some();
    listed.push(json!({
      "proposal": seq,
      "tool": action.tool,
      "task": action.task,
      "proposed": proposed,
      "click_only": click_only,
    }));
  }
  Ok(listed)
}

fn refused(code: &str, key: &'static str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key)])
}
