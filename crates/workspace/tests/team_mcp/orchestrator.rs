//! A team task's run, which this computer keeps no definition of, where the app and the Orchestrator chat read a run's
//! task: it is under way for the sidebar and Stop auto-approving, its task is told by the key its inputs kept, the
//! chat merges it on its own by the rule of a local run, unless the team server holds its task blocked, and the local
//! merge takes it into its module's branch, never the base. Its task is in the module its run started in, so a
//! conversation that watches the module leads its events (OA-28).

use std::collections::HashSet;

use orchester_core::{
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  integration::base::RunBaseSource,
  orchestrator::{
    ActionOutcome, AgentWait, Chat, ChatAction, EntryKind, MergeLeft, Merging, RunEnd, Subscription, TurnState,
    WaitKind, WatchTarget,
    watch::{NamedTarget, OpenTask, OpenWork},
  },
  run::RunStatus,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  orchestrator::{
    auto_merge::{completes_in, left_message, left_on_server, merging_in},
    waits::{Switches, check_merges_in, follow_in},
    watch::{StopBy, StopWhat, stop_in, watch_in},
  },
  tasks::{
    active_runs,
    integration::{IntegrationProblem, merge_in},
    recorded,
    run_approvals::stop_auto_approving_in,
    worktree_removal::WorktreeRemover,
  },
};

use crate::{
  team_runs::{started, started_without_record},
  unrecorded::stream_era,
};

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
fn team_mcp_the_chat_merges_a_succeeded_team_run_by_the_rule_of_a_local_one() {
  let team = started();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let db = team.checkout.db();
  assert_eq!(
    *db.run(team.run).expect("read").expect("run").status(),
    RunStatus::Succeeded
  );

  assert_eq!(
    merging_in(&db, team.run).expect("rule"),
    Merging::Merges,
    "into its module's branch, as a local run into its base"
  );
  assert!(
    !completes_in(&db, team.run).expect("rule"),
    "a run with nothing to merge is not completed from here: the server closes its task on the run's report"
  );
}

#[test]
fn team_mcp_the_chat_leaves_a_team_run_whose_task_the_server_holds_blocked() {
  let team = started();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let run = team.run.to_string();
  let asked = || {
    crate::team_cache::keep_cached_team(&team.checkout.workspace, team.checkout.root());
    team
      .checkout
      .runtime
      .block_on(left_on_server(&team.checkout.services, ("Kontur", &run)))
      .unwrap_or_else(|error| panic!("{}", error.to_json(|message| message.key.to_owned())))
  };
  assert_eq!(asked(), None, "the server does not hold the task back");

  // The rule read from this computer's records still says it merges: only the server knows the mark.
  team.seen.lock().task_fields = serde_json::json!({"blocked_reason": "Waiting for the staging key"});
  assert_eq!(
    merging_in(&team.checkout.db(), team.run).expect("rule"),
    Merging::Merges
  );
  assert_eq!(
    asked(),
    Some(MergeLeft::MarkedBlocked {
      reason: "Waiting for the staging key".to_owned()
    })
  );
}

#[test]
fn team_mcp_the_chat_says_a_team_run_with_no_team_record_can_be_neither_merged_nor_delivered() {
  // The start stopped before the record was written: the run and its team inputs are there.
  let team = started_without_record();
  let db = team.checkout.db();
  assert_eq!(
    merging_in(&db, team.run).expect("rule"),
    Merging::Left {
      why: MergeLeft::TeamUnrecorded
    }
  );
  assert!(!completes_in(&db, team.run).expect("rule"), "no completion either");

  // What the merge tool tells the chat when it is called in a turn nobody asked for.
  let told = left_message(&MergeLeft::TeamUnrecorded, 3);
  assert_eq!(told.key, "orchestrator.tools.mergeLeft.teamUnrecorded");
  assert_eq!(told.args, vec![("run", "3".to_owned())]);
}

#[test]
fn team_mcp_the_chat_judges_a_stream_era_run_as_a_local_run() {
  let (checkout, _server, _task, run) = stream_era();
  let db = checkout.db();
  // Its task is cached with the team's identity, yet the run is read by the local rules: it has not succeeded.
  assert_eq!(
    merging_in(&db, run).expect("rule"),
    Merging::Left {
      why: MergeLeft::NotReady
    }
  );
}
/// The work was done on this computer and the module's branch is this checkout's: the task page's merge takes a team
/// run into the branch it started from, as it takes a local run into its base, and the base itself is left to the
/// module's delivery.
#[test]
fn team_mcp_the_local_merge_takes_a_team_run_into_its_modules_branch_and_leaves_the_base() {
  let team = started();
  let root = team.checkout.root();
  let started = team.checkout.db().run(team.run).expect("read").expect("the run");
  let module_branch = started.base_branch().expect("the module's branch").as_str().to_owned();
  let module_branch = module_branch.as_str();
  // The module's branch where the run started, and what the run's agent committed on the run's own branch.
  crate::git(root, &["branch", module_branch, started.base().as_str()]);
  crate::git(root, &["switch", "-c", started.branch().as_str(), module_branch]);
  std::fs::write(root.join("retry.txt"), "retry\n").expect("the run's file");
  crate::git(root, &["add", "retry.txt"]);
  crate::git(root, &["commit", "-m", "KON-7: retry"]);
  let work = crate::git(root, &["rev-parse", "HEAD"]);
  crate::git(root, &["switch", "main"]);
  let base = crate::git(root, &["rev-parse", "main"]);
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let programs = Programs::processes();
  let worktrees = WorktreeRemover::here_for_test(root, programs.clone());

  let merged = merge_in(&worktrees, &programs, (team.run, None)).map(|merged| merged.merge);
  let merge = merged.as_ref().expect("merged into the module's branch");
  assert_eq!(merge.run, team.run);
  // The module's branch holds the run's work; the base and what the checkout has checked out are as they were.
  crate::git(root, &["merge-base", "--is-ancestor", &work, module_branch]);
  assert_eq!(crate::git(root, &["rev-parse", "main"]), base);
  assert_eq!(crate::git(root, &["branch", "--show-current"]), "main");
  assert!(!root.join("retry.txt").exists(), "the checkout's files are untouched");
  assert!(
    team.checkout.db().run_merge(team.run).expect("read").is_some(),
    "recorded"
  );
}

/// A team run whose start was never recorded has no claim or work it ran under, and its merge is not tried: no attempt
/// is recorded for the press.
#[test]
fn team_mcp_no_local_merge_takes_a_team_run_with_no_team_record() {
  let team = started_without_record();
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));
  let programs = Programs::processes();
  let worktrees = WorktreeRemover::here_for_test(team.checkout.root(), programs.clone());

  let refused = merge_in(&worktrees, &programs, (team.run, None)).map(|merged| merged.merge);
  assert!(
    matches!(&refused, Err(IntegrationProblem::Store(message)) if message.key == "localIntegration.teamRun"),
    "{refused:?}"
  );
  let db = team.checkout.db();
  assert!(db.merge_attempts(team.run).expect("attempts").is_empty());
  assert!(db.open_merge_attempts().expect("open").is_empty());
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

/// The waits `chat` keeps.
fn waits_of(team: &crate::team_runs::Started, chat: &Chat) -> Vec<AgentWait> {
  let entries = team.checkout.db().chat_entries(chat.id()).expect("entries");
  entries
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Wait { wait, .. } => Some(wait),
      _ => None,
    })
    .collect()
}

#[test]
fn team_mcp_a_module_subscription_leads_the_events_of_a_team_task_in_that_module() {
  let team = started();
  let mut db = team.checkout.db();
  let run = db.run(team.run).expect("read").expect("run");
  let RunBaseSource::Module { module } = run.base_source() else {
    panic!("the team run started on its module's branch: {:?}", run.base_source());
  };
  let task = recorded::task_of(&db, run.task())
    .expect("read")
    .expect("recorded by its run");
  assert_eq!(
    task.module(),
    Some(module),
    "the task is in the module its run started in"
  );

  // One conversation watches the module, another the tasks without a module.
  let leading = Chat::named(ChatId::generate(), "Webhooks v2", at(10));
  let elsewhere = Chat::named(ChatId::generate(), "The rest", at(10));
  for (chat, target) in [
    (&leading, WatchTarget::Module(module)),
    (&elsewhere, WatchTarget::WithoutModule),
  ] {
    let person = ChatMessageId::generate();
    let text = MessageBody::new("Look after it").expect("text");
    db.keep_chat_message(chat, (person, &text), at(10)).expect("kept");
    db.end_chat_turn(person, &TurnState::Answered, at(11)).expect("ended");
    db.start_subscription(&Subscription::new(chat.id(), target, "", at(12)))
      .expect("watching");
  }
  let project = db.project().id();
  drop(db);
  team.step_into(|run, execution, now| run.complete_step(execution, now).expect("passed"));

  let (root, programs) = (team.checkout.root(), Programs::processes());
  let checks = check_merges_in((root, &programs), &HashSet::new()).expect("checked");
  follow_in((root, project, &programs), &checks, (Switches { drives: false }, true)).expect("followed");

  let led = waits_of(&team, &leading);
  assert_eq!(led.len(), 1, "the module's conversation keeps the run's end: {led:?}");
  assert_eq!((led[0].task.as_str(), led[0].run), ("KON-7", team.run));
  assert!(
    waits_of(&team, &elsewhere).is_empty(),
    "a task in a module is no task without a module"
  );
}

#[test]
fn team_mcp_a_watch_on_a_team_task_kept_only_as_an_identity_is_not_gone_and_names_its_open_work() {
  let team = started();
  let task = team.checkout.db().run(team.run).expect("read").expect("run").task();
  let (root, chat) = (team.checkout.root(), Chat::named(ChatId::generate(), "KON-7", at(10)));
  let given = NamedTarget {
    target: WatchTarget::Task(task),
    name: None,
  };

  // The task has no definition here, only the key its run's inputs kept: it exists, and is named by that key.
  let watched = watch_in(root, &chat, std::slice::from_ref(&given), "").expect("watched");
  assert!(watched.gone.is_empty(), "{:?}", watched.gone);
  assert_eq!(watched.started.len(), 1);
  assert_eq!(watched.started[0].name.as_deref(), Some("KON-7"));

  // Stopping it names the run that goes on, which the stop cancels nothing of.
  let stopped = stop_in(root, &chat, &StopWhat::All, StopBy::Unwatch).expect("stopped");
  assert_eq!(
    stopped.left_open,
    vec![OpenTask {
      task: "KON-7".to_owned(),
      work: OpenWork::Runs
    }]
  );
}

#[test]
fn team_mcp_all_with_runs_gives_a_team_task_as_all_does_with_the_runs_its_record_was_read_from() {
  let team = started();
  let db = team.checkout.db();

  let with_runs = recorded::all_with_runs(&db).expect("tasks with their runs");
  assert_eq!(with_runs.len(), 1);
  let (task, runs) = &with_runs[0];
  assert_eq!(recorded::all(&db).expect("tasks"), vec![task.clone()]);
  assert_eq!(task.key().to_string(), "KON-7");
  let ids: Vec<_> = runs
    .iter()
    .map(|listed| listed.as_ref().expect("readable").id())
    .collect();
  assert_eq!(ids, vec![team.run]);
}
