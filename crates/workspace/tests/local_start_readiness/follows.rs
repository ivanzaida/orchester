//! The other starts of a run on a pipeline that is not ready here (ORC-840): a message that runs the task again (the
//! MCP `send_message`), Fix CI by hand and automatically, and a reviewed group queued together. None queues, claims or
//! starts anything; a message and a Fix CI stay stored and queued, and the run says why on the task.

use orchester_core::{
  id::IdempotencyKey,
  pull_request::ci::{
    CiCheck, CiChecks, CiState,
    fix::{AutoFix, FixEventKind, FixLimit, FixedBy},
  },
  run::Answerer,
  store::{ConversationStore, RunStore},
  task::TaskLimits,
  task_group::{ReviewedPipeline, Scheduling, TaskGroup},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_workspace::{
  app_mcp::{self, Translate},
  pull_requests::{
    ci::ReadCi,
    ci_fix::{FixDone, Fixer, fix_ci, set_auto_fix_in},
  },
  secrets::Vault,
  tasks::{conversation::follow::Sent, group},
};
use serde_json::{Value, json};

use super::{Kontur, not_ready};

fn failed() -> ReadCi {
  ReadCi {
    number: 142,
    checks: CiChecks {
      checks: vec![CiCheck {
        name: "lint".to_owned(),
        workflow: Some("CI".to_owned()),
        state: CiState::Failed,
        url: Some("https://github.com/ivanzaida/kontur/actions/runs/900/job/9002".to_owned()),
        job: Some(9002),
        log_tail: None,
      }],
    },
    read_at: Timestamp::now(),
  }
}

impl Kontur {
  fn send_message(&self) -> Value {
    let render = |message: &orchester_workspace::messages::Message| message.key.to_owned();
    let translate = Translate::new(render);
    let Value::Object(args) = json!({ "project": "Kontur", "run": "1", "text": "Try it again with a smaller batch." })
    else {
      unreachable!("an object");
    };
    let result = self
      .runtime
      .block_on(app_mcp::call(&self.services, &translate, "send_message", args))
      .expect("a tool");
    result.unwrap_or_else(|error| error.to_json(render))
  }

  /// Fix CI of the failing `lint` check on the run that ended, as `fixer` presses it.
  fn fix_ci(&self, fixer: Fixer) -> (FixDone, Option<Sent>) {
    let (runners, servers, open) = &self.follow;
    let task = self.task().id();
    let run = self.db().runs_of_task(task).expect("runs")[0]
      .as_ref()
      .expect("the run reads")
      .id();
    self
      .runtime
      .block_on(fix_ci(
        (runners, servers),
        (&self.workspace, &Vault::keychain()),
        open,
        (task, run),
        (failed(), false),
        fixer,
      ))
      .expect("Fix CI answers")
  }

  fn fix_events(&self) -> Vec<FixEventKind> {
    let run = self.db().runs_of_task(self.task().id()).expect("runs")[0]
      .as_ref()
      .expect("the run reads")
      .id();
    self
      .db()
      .ci_fixes(run)
      .expect("the run's Fix CI")
      .events
      .into_iter()
      .map(|event| event.kind)
      .collect()
  }
}

#[test]
fn local_start_send_message_that_runs_the_task_again_is_refused_before_it_is_stored_so_a_retry_stores_it_once() {
  let (kontur, program, _outside) = not_ready();
  kontur.end_a_run();
  let before = kontur.standing();
  let stored = || kontur.db().messages(kontur.task().id()).expect("messages").len();
  let first = stored();

  let refused = kontur.send_message();
  assert_eq!(refused["error"]["code"], "not_ready_here", "{refused}");
  assert_eq!(
    refused["error"]["reasons"],
    json!(["notReadyHere.refused", "notReadyHere.localMcpProgramNotFound"])
  );
  assert_eq!(kontur.standing(), before, "no run, no attempt, nothing queued");
  assert_eq!(
    stored(),
    first,
    "nothing was stored, so the client knows it must send it again"
  );
  assert_eq!(
    kontur.send_message()["error"]["code"],
    "not_ready_here",
    "asked again, still refused, still nothing stored"
  );
  assert_eq!(stored(), first);

  std::fs::write(&program, "#!/bin/sh\n").expect("the program appears");
  let sent = kontur.send_message();
  assert_ne!(
    sent["error"]["code"], "not_ready_here",
    "ready, the message goes on: {sent}"
  );
  assert_eq!(stored(), first + 1, "the retry stored the message once");
}

#[test]
fn local_start_fix_ci_by_hand_starts_nothing_and_records_that_the_message_waits() {
  let (kontur, _program, _outside) = not_ready();
  kontur.end_a_run();
  let before = kontur.standing();

  let (done, sent) = kontur.fix_ci(Fixer::Hand { by: Answerer::Person });
  assert!(matches!(done, FixDone::Stored(_)), "{done:?}");
  assert!(matches!(sent, Some(Sent::NotReady(_))), "{sent:?}");
  assert_eq!(kontur.standing(), before, "no run, no attempt");
  let events = kontur.fix_events();
  assert!(
    matches!(
      events.as_slice(),
      [
        FixEventKind::Sent { by: FixedBy::Hand, .. },
        FixEventKind::NotReady { .. }
      ]
    ),
    "the task says it: {events:?}"
  );
}

#[test]
fn local_start_fix_ci_automatically_records_a_reason_and_does_not_send_again() {
  let (kontur, _program, _outside) = not_ready();
  kontur.end_a_run();
  let task = kontur.task().id();
  let run = kontur.db().runs_of_task(task).expect("runs")[0]
    .as_ref()
    .expect("the run reads")
    .id();
  let on = AutoFix {
    on: true,
    limit: FixLimit::of(3).expect("a limit"),
  };
  set_auto_fix_in(kontur.dir.path(), run, on, Answerer::Person).expect("the switch");
  let before = kontur.standing();

  let (done, sent) = kontur.fix_ci(Fixer::Automatic);
  assert!(matches!(done, FixDone::Stored(_)), "{done:?}");
  assert!(matches!(sent, Some(Sent::NotReady(_))), "{sent:?}");
  assert_eq!(kontur.standing(), before, "no run, no attempt");
  assert!(
    matches!(
      kontur.fix_events().as_slice(),
      [
        FixEventKind::Sent {
          by: FixedBy::Automatically { count: 1, .. },
          ..
        },
        FixEventKind::NotReady { .. }
      ]
    ),
    "a visible reason, not a silent skip: {:?}",
    kontur.fix_events()
  );

  let (again, _) = kontur.fix_ci(Fixer::Automatic);
  assert_eq!(
    again,
    FixDone::Nothing,
    "the failure was answered: it is not sent every minute"
  );
}

#[test]
fn local_start_a_group_with_a_member_that_is_not_ready_is_not_queued_and_names_it() {
  let (kontur, program, _outside) = not_ready();
  let task = kontur.task().id();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    vec![task],
    Scheduling::Sequential,
    TokenBudget::new(2_000_000).expect("cap"),
  )
  .expect("group");
  let reviewed = [ReviewedPipeline {
    task,
    pipeline: kontur.published.pipeline,
    version: kontur.published.snapshot.pipeline().number,
  }];
  let queue = || {
    kontur.runtime.block_on(group::queue(
      (&kontur.workspace, &Vault::keychain()),
      kontur.dir.path(),
      &group,
      TaskLimits::NONE,
      &reviewed,
    ))
  };

  let refused = queue().expect_err("the member's pipeline is not ready here");
  assert_eq!(refused.len(), 1);
  assert_eq!(refused[0].key, "notReadyHere.groupMember");
  assert_eq!(
    refused[0].args,
    [("task", "KON-1".to_owned()), ("pipeline", "Quick fix".to_owned())]
  );
  assert!(
    kontur.db().task_group(group.key()).expect("read").is_none(),
    "nothing was queued"
  );

  std::fs::write(&program, "#!/bin/sh\n").expect("the program appears");
  assert!(queue().is_ok(), "ready, the group is queued");
}
