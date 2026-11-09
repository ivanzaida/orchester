//! A team task's run, which this computer keeps no definition of, where the app and the Orchestrator chat read a run's
//! task: it is under way for the sidebar and Stop auto-approving, its task is told by the key its inputs kept, the
//! chat leaves its delivery to the person, and no local merge takes it.

use std::collections::HashSet;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, MergeLeft, Merging, RunEnd, Subscription, TurnState,
    WaitKind, WatchTarget,
  },
  run::RunStatus,
  store::RunStore,
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  orchestrator::{
    auto_merge::{completes_in, merging_in},
    waits::{Switches, check_merges_in, follow_in},
  },
  tasks::{
    active_runs,
    integration::{IntegrationProblem, merge_in},
    run_approvals::stop_auto_approving_in,
    worktree_removal::WorktreeRemover,
  },
};

use crate::team_runs::started;

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

#[test]
fn team_mcp_a_running_team_run_is_under_way_and_stops_with_auto_approving() {
  let team = started();
  let root = team.checkout.root();

  let active = active_runs::load(root).expect("active runs");
  assert_eq!(active.len(), 1, "{active:?}");
  assert_eq!(active[0].run, team.run);
  assert_eq!(active[0].key.to_string(), "KON-7");

  team
    .checkout
    .db()
    .set_run_auto_approve(team.run, Some(true))
    .expect("auto-approve");
  let stopped = stop_auto_approving_in(root).expect("stopped");
  assert_eq!(stopped, vec![team.run], "the team run auto-approved and stopped");
}

#[test]
fn team_mcp_the_chat_leaves_a_succeeded_team_run_to_the_person() {
  let team = started();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let db = team.checkout.db();
  assert_eq!(
    *db.run(team.run).expect("read").expect("run").status(),
    RunStatus::Succeeded
  );

  assert_eq!(
    merging_in(&db, team.run).expect("rule"),
    Merging::Left {
      why: MergeLeft::TeamDelivery
    }
  );
  assert!(!completes_in(&db, team.run).expect("rule"), "no completion either");
}

#[test]
fn team_mcp_no_local_merge_takes_a_team_run() {
  let team = started();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let programs = Programs::processes();
  let worktrees = WorktreeRemover::here_for_test(team.checkout.root(), programs.clone());

  let refused = merge_in(&worktrees, &programs, (team.run, None)).map(|merged| merged.merge);
  assert!(
    matches!(&refused, Err(IntegrationProblem::Store(message)) if message.key == "localIntegration.teamRun"),
    "{refused:?}"
  );
}

#[test]
fn team_mcp_the_chat_keeps_the_end_of_a_team_run_in_the_conversation_that_leads_its_task() {
  let team = started();
  let mut db = team.checkout.db();
  let task = db.run(team.run).expect("read").expect("run").task();
  // A conversation that updated the team task and watches it from then on.
  let chat = Chat::named(ChatId::generate(), "KON-7", at(10));
  let person = ChatMessageId::generate();
  let text = MessageBody::new("Look after it").expect("text");
  db.keep_chat_message(&chat, (person, &text), at(10)).expect("kept");
  db.add_chat_action(
    chat.id(),
    &ChatAction::new("update_task", Some("KON-7".to_owned()), ActionOutcome::Done),
    at(11),
  )
  .expect("kept");
  db.end_chat_turn(person, &TurnState::Answered, at(12)).expect("ended");
  db.start_subscription(&Subscription::new(chat.id(), WatchTarget::Task(task), "", at(13)))
    .expect("watching");
  let project = db.project().id();
  drop(db);
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));

  let (root, programs) = (team.checkout.root(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, project, &programs), &checks, (Switches { drives: false }, true)).expect("followed");

  let waits: Vec<AgentWait> = team
    .checkout
    .db()
    .chat_entries(chat.id())
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } => Some(wait),
      _ => None,
    })
    .collect();
  assert_eq!(waits.len(), 1, "{waits:?}");
  assert_eq!(waits[0].task, "KON-7");
  assert_eq!(waits[0].run, team.run);
  assert_eq!(waits[0].kind, WaitKind::Ended(RunEnd::Succeeded));
}
