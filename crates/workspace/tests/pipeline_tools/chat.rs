//! The Orchestrator chat changes pipelines only while *Let the Orchestrator drive* is on (D-033, PV2-30): while it is
//! off, `create_pipeline`, `save_pipeline` and `duplicate_pipeline` are refused as `drive_only` with words that tell
//! the agent to say what to change, nothing is stored and the refusal is kept; while it is on they publish as for any
//! client. A publication of a pipeline kept before pipelines lost their drafts reads, applying it is refused as
//! changed since it was proposed, publishing nothing, and one an earlier session claimed is never read as carried out.

use orchester_core::{
  orchestrator::{
    ActionOutcome, ChatAction, EntryKind,
    change::{Change, ProposedChange, Publishing},
    instructions::{ProjectReads, system_prompt},
  },
  time::Timestamp,
};
use orchester_workspace::orchestrator::{
  facts,
  proposal::{Decision, decide},
  recover::recover_in,
  tools::listed,
};
use serde_json::{Value, json};

use super::{Desktop, reasons, step};

/// Delivery, created as v1 by an MCP client a person let widen, with one step on Builder.
fn delivery(desktop: &Desktop) {
  desktop.lets_automation_widen(true);
  desktop.done(
    "create_pipeline",
    json!({ "project": "Kontur", "name": "Delivery", "from": "empty", "steps": [step("plan", "Builder", &[])] }),
  );
}

/// Each of the chat's pipeline changes, with arguments that would publish.
fn changes() -> [(&'static str, Value); 3] {
  let mut plan = step("plan", "Builder", &[]);
  plan["budget"] = json!(1000);
  [
    (
      "save_pipeline",
      json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [plan] }),
    ),
    (
      "create_pipeline",
      json!({ "project": "Kontur", "name": "Review", "from": "empty", "steps": [step("review", "Reviewer", &[])] }),
    ),
    (
      "duplicate_pipeline",
      json!({ "project": "Kontur", "pipeline": "Delivery", "name": "Delivery copy" }),
    ),
  ]
}

#[test]
fn pipeline_tools_chat_without_drive_changes_no_pipeline_and_says_what_to_do() {
  let desktop = Desktop::new();
  delivery(&desktop);
  desktop.says("Make Delivery stricter");
  for (name, arguments) in changes() {
    let refused = desktop.chat_refused(name, arguments);
    assert_eq!(refused["code"], "drive_only", "{name}: {refused}");
    assert_eq!(reasons(&refused), ["orchestrator.tools.pipelinesDriveOnly"], "{name}");
    let kept = desktop.last_action();
    assert_eq!(kept.tool, name);
    assert!(
      matches!(&kept.outcome, ActionOutcome::Refused { code, .. } if code == "drive_only"),
      "{name}: the refusal is kept, not proposed: {:?}",
      kept.outcome
    );
    assert!(kept.change.is_none(), "{name}: no card to apply");
  }
  assert_eq!(
    desktop.pipelines(),
    vec![("Delivery".to_owned(), 1, false)],
    "nothing was stored"
  );
  for tool in listed(false)
    .into_iter()
    .filter(|tool| ["create_pipeline", "save_pipeline", "duplicate_pipeline"].contains(&tool.name.as_ref()))
  {
    let told = tool.description.as_deref().unwrap_or_default();
    assert!(
      told.contains("drive_only") && told.contains("Tell the person what to change"),
      "{}: {told}",
      tool.name
    );
  }
  let prompt = system_prompt("Kontur", desktop.scope.project, ProjectReads::FileTools);
  assert!(prompt.contains("While it is off you change no pipeline"), "{prompt}");
  assert!(!prompt.contains("publish_pipeline"), "the prompt names no gone tool");
}

#[test]
fn pipeline_tools_chat_while_driving_saves_creates_and_duplicates() {
  let desktop = Desktop::new();
  delivery(&desktop);
  desktop.drives(true);
  desktop.says("Make Delivery stricter, add Review and copy Delivery");
  for (name, arguments) in changes() {
    let told = desktop.chat(name, arguments);
    assert!(!told.to_string().contains("draft"), "{name}: {told}");
    assert_eq!(desktop.last_action().outcome, ActionOutcome::Done, "{name}");
  }
  let mut stored = desktop.pipelines();
  stored.sort();
  assert_eq!(
    stored,
    vec![
      ("Delivery".to_owned(), 2, false),
      ("Delivery copy".to_owned(), 1, false),
      ("Review".to_owned(), 1, false),
    ]
  );
  // What widens what agents may do still needs a person's switch, while the chat drives too.
  desktop.lets_automation_widen(false);
  let widens = desktop.chat_refused(
    "save_pipeline",
    json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [step("plan", "Reviewer", &[])] }),
  );
  assert_eq!(widens["code"], "person_only", "{widens}");
}

/// A proposal to publish Delivery as `version`, kept in the conversation as the chat kept one before pipelines lost
/// their drafts, waiting for the person; it and its place in the conversation.
fn kept_publication(desktop: &Desktop, version: u32) -> (ChatAction, i64) {
  let kept = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Delivery".to_owned(),
      change: Change::Publish {
        of: Publishing::Pipeline,
        resource: desktop.pipeline("Delivery"),
        version,
        saved: Timestamp::now(),
      },
    })),
    ..ChatAction::new("publish_pipeline", None, ActionOutcome::Waits)
  };
  let mut db = desktop.db();
  db.add_chat_action(desktop.scope.chat, &kept, Timestamp::now())
    .expect("kept as before");
  let seq = db
    .chat_entries(desktop.scope.chat)
    .expect("entries")
    .into_iter()
    .rev()
    .find(|entry| matches!(entry.kind, EntryKind::Action(_)))
    .expect("the old proposal")
    .seq;
  (kept, seq)
}

#[test]
fn pipeline_tools_chat_publication_kept_from_drafts_reads_stale_and_publishes_nothing() {
  let desktop = Desktop::new();
  delivery(&desktop);
  desktop.says("Publish Delivery");
  let (kept, seq) = kept_publication(&desktop, 2);
  let read = facts::load_in(
    &desktop.scope.root,
    &orchester_exec::programs::Programs::processes(),
    &kept,
  );
  assert!(
    matches!(read, Ok(Some(facts::Facts::Publication(_)))),
    "its card reads: {read:?}"
  );
  assert_eq!(
    Publishing::of_tool("publish_pipeline"),
    None,
    "no tool publishes a pipeline"
  );
  let translate = orchester_workspace::app_mcp::Translate::new(|message| message.key.to_owned());
  desktop
    .runtime
    .block_on(decide(
      (&desktop.services, &translate, &desktop.runners),
      &desktop.scope,
      seq,
      Decision::Accept { widens: Vec::new() },
    ))
    .expect("the decision is kept");
  let settled = desktop.last_action();
  assert!(
    matches!(&settled.outcome, ActionOutcome::Refused { code, reason }
      if code == "changed_since_proposed" && reason == "orchestrator.proposal.pipelineNoDraft"),
    "{:?}",
    settled.outcome
  );
  assert_eq!(
    desktop.pipelines(),
    vec![("Delivery".to_owned(), 1, false)],
    "nothing was published"
  );
}

#[test]
fn pipeline_tools_chat_publication_kept_from_drafts_is_never_recovered_as_applied() {
  let desktop = Desktop::new();
  delivery(&desktop);
  desktop.says("Publish Delivery");
  // Claimed by a person's Apply in a session that ended before it settled; Delivery stands at v2 since, saved by
  // someone else, which is the version the old proposal named.
  let (_, seq) = kept_publication(&desktop, 2);
  let claimed_at = Timestamp::now();
  desktop
    .db()
    .claim_chat_action_approved((desktop.scope.chat, seq), None, claimed_at)
    .expect("claimed")
    .expect("it waited");
  let mut plan = step("plan", "Builder", &[]);
  plan["budget"] = json!(1000);
  desktop.done(
    "save_pipeline",
    json!({ "project": "Kontur", "pipeline": "Delivery", "steps": [plan] }),
  );
  assert_eq!(desktop.pipelines(), vec![("Delivery".to_owned(), 2, false)]);
  let translate = orchester_workspace::app_mcp::Translate::new(|message| message.key.to_owned());
  let next_session = Timestamp::from_unix_millis(claimed_at.unix_millis() + 60_000);
  let settled = recover_in(&desktop.scope.root, next_session, &translate).expect("recovered");
  assert_eq!(settled, 1, "the claim is settled");
  let recovered = desktop.last_action();
  assert!(
    matches!(&recovered.outcome, ActionOutcome::Refused { code, .. } if code == "interrupted"),
    "a later save is not this decision's: {:?}",
    recovered.outcome
  );
}
