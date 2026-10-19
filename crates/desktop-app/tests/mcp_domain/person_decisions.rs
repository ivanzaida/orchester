//! Drives the real controls headless, then reads the decisions the real runner stored.
mod regressions;
use orchester_app::routes::{ProjectPage, Route, TaskView};
use orchester_core::{
  id::RunId,
  run::{Answerer, RunEventKind},
  store::{ConversationStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::Served;

struct Fixture {
  app: Served,
  repo: std::path::PathBuf,
  run: RunId,
}

impl Fixture {
  fn new(scenario: &str, title: &str, view: TaskView) -> Self {
    if title == "Add serde to the webhook payloads" {
      return Self::approval(view);
    }
    let programs = super::runner_programs::programs();
    let (runner, fake) = (programs.runner.clone(), programs.fake_codex.clone());
    let (folder, seeded) = Served::seed(scenario, Some(fake));
    let key = seeded
      .tasks
      .iter()
      .find(|task| task.title == title)
      .expect("seeded task")
      .key
      .clone();
    let mut db = ProjectDb::open(&seeded.repo).expect("project");
    let task = db
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("task");
    let run = db
      .runs_of_task(task.id())
      .expect("runs")
      .pop()
      .expect("run")
      .expect("readable")
      .id();
    if title == "Pick where the docs button goes" {
      Self::question_at_step_end(&mut db, run);
    }
    drop(db);
    let mut app = Served::start_with(folder, &seeded.app_database, runner, true);
    app.link.navigation().expect("navigation").go(Route::Project(
      seeded.project,
      ProjectPage::Task(key.parse().expect("key"), view),
    ));
    app.until("the runner online", |app| {
      matches!(
        app.services().runners.status().state(),
        orchester_workspace::runner::state::RunnerState::Online
      )
    });
    Self {
      app,
      repo: seeded.repo,
      run,
    }
  }

  /// A step-end question survives attaching a runner; an abandoned in-turn seed request correctly does not.
  fn question_at_step_end(db: &mut ProjectDb, id: RunId) {
    use orchester_core::{
      connection::stream::InvocationOutcome,
      question::{QuestionRequest, QuestionTurn},
      run::WaitReason,
      store::ActivityStore,
      time::Timestamp,
    };
    let asked = db.questions(id).expect("questions").pop().expect("question").request;
    let now = Timestamp::now();
    let mut run = db.run(id).expect("run").expect("run");
    let events = run.resume_step(asked.execution(), now).expect("resume seeded hold");
    db.append_events(&run, &events).expect("resume");
    let events = run
      .wait_step(asked.execution(), WaitReason::Message, now)
      .expect("waiting for an answer");
    db.append_events(&run, &events).expect("wait");
    let mut invocation = db
      .invocations(id)
      .expect("invocations")
      .into_iter()
      .find(|invocation| invocation.id == asked.invocation())
      .expect("asking invocation");
    invocation.ended_at = Some(now);
    invocation.outcome = Some(InvocationOutcome::Completed);
    invocation.timing.resume(now);
    db.save_invocation(&invocation).expect("turn ended");
    db.end_waiting_question(id, asked.id())
      .expect("old in-turn request ended");
    let request = QuestionRequest::new(
      format!("{}-step-end", asked.id()),
      (id, asked.execution(), asked.invocation()),
      asked.agent().to_owned(),
      asked.questions().to_vec(),
      (now, QuestionTurn::StepEnd),
    )
    .expect("step-end request");
    db.ask_questions(&request).expect("question waits at step end");
  }

  fn approval(view: TaskView) -> Self {
    Self::live_decision("Escalate `git status`.", view)
  }

  /// Live runs retain the result's revision, which accepting correctly requires.
  fn live_decision(brief: &str, view: TaskView) -> Self {
    let (mut app, repo, fake) = super::live::served(true);
    super::live::set_up(&mut app, &repo, &fake);
    app.let_automation_widen(false);
    let key = super::live::run(&mut app, brief);
    let listed = app.ok("get_run", &json!({"project":"Kontur","run":"1"}));
    let run = listed["id"].as_str().expect("run id").parse().expect("run id");
    let db = ProjectDb::open(&repo).expect("project");
    app.until("the agent's decision request", |_| {
      if brief.starts_with("Escalate") {
        !db.pending_tool_approvals(run).expect("requests").is_empty()
      } else {
        db.run(run)
          .expect("run")
          .is_some_and(|run| matches!(run.status(), orchester_core::run::RunStatus::WaitingForPerson(_)))
      }
    });
    let project = app.services().workspace.projects().expect("projects")[0].id;
    app.link.navigation().expect("navigation").go(Route::Project(
      project,
      ProjectPage::Task(key.parse().expect("key"), view),
    ));
    Self { app, repo, run }
  }

  fn card_button(&mut self, prefix: &str, suffix: &str) -> String {
    let id = std::cell::RefCell::new(None);
    self.app.until("card button", |app| {
      app
        .tree
        .find_element(|element| {
          if let Some(matched) = element.id().filter(|id| id.starts_with(prefix) && id.ends_with(suffix)) {
            *id.borrow_mut() = Some(matched.to_owned());
            return true;
          }
          false
        })
        .is_some()
    });
    id.into_inner().expect("button")
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("project")
  }

  fn consent(&self) {
    self
      .app
      .services()
      .workspace
      .set_automation_answers_approvals(true)
      .expect("consent");
    assert!(
      !self.app.services().workspace.automation_permission_consent(),
      "one grant is sufficient"
    );
  }

  fn press(&mut self, button: &str, by: Answerer) {
    self.app.until(button, |app| app.shows(button));
    match by {
      Answerer::Person => self.app.click_by_person(button),
      Answerer::Automation => {
        self
          .app
          .click_by_client(button)
          .expect("client input reaches the handler");
      }
    }
  }

  fn plan_by(&self) -> Option<Answerer> {
    self
      .db()
      .plans(self.run)
      .expect("plans")
      .iter()
      .find_map(|plan| plan.review.as_ref().map(|review| review.by))
  }

  fn stuck_by(&self) -> Option<Answerer> {
    self
      .db()
      .events(self.run)
      .expect("events")
      .iter()
      .rev()
      .find_map(|event| match event.kind() {
        RunEventKind::StuckDecided { by, .. } => Some(*by),
        _ => None,
      })
  }

  fn refused_then_consent(&mut self, button: &str, refusal: &str) {
    let before = (self.plan_by(), self.stuck_by());
    self.press(button, Answerer::Automation);
    self.app.until("synthetic decision refused", |app| app.shows(refusal));
    assert_eq!((self.plan_by(), self.stuck_by()), before);
    self.consent();
  }
}

#[test]
fn person_decisions_summary_plan_buttons_refuse_and_record_the_decider() {
  for button in ["plan-approve", "plan-request-changes"] {
    for by in [Answerer::Person, Answerer::Automation] {
      let mut f = Fixture::new("task-states", "Rate-limit messages from GitHub", TaskView::Summary);
      f.app.until("plan bar online", |app| {
        app.shows(button) && !app.shows("plan-review-offline")
      });
      if button == "plan-request-changes" {
        f.app.set_by_client("plan-review-note", "Keep the retry bounded.");
      }
      if by == Answerer::Automation {
        f.refused_then_consent(button, "plan-review-person-only");
      }
      f.press(button, by);
      let db = f.db();
      let run = f.run;
      f.app.until("plan decision stored", |_| {
        db.plans(run)
          .expect("plans")
          .iter()
          .any(|plan| plan.review.as_ref().is_some_and(|review| review.by == by))
      });
      assert_eq!(f.plan_by(), Some(by));
    }
  }
}

#[test]
fn person_decisions_composer_plan_changes_refuse_and_record_the_decider() {
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("task-states", "Rate-limit messages from GitHub", TaskView::Conversation);
    f.app.until("composer", |app| app.shows("composer-input"));
    f.app.set_by_client("composer-input", "Keep the retry bounded.");
    if by == Answerer::Automation {
      f.refused_then_consent("composer-send", "composer-failed");
    }
    f.press("composer-send", by);
    let db = f.db();
    let run = f.run;
    f.app.until("plan changes stored", |_| {
      db.plans(run)
        .expect("plans")
        .iter()
        .any(|plan| plan.review.as_ref().is_some_and(|review| review.by == by))
    });
    assert_eq!(f.plan_by(), Some(by));
  }
}

#[test]
fn person_decisions_retry_return_stop_and_accept_refuse_and_record_the_decider() {
  for button in [
    "task-allow-retry",
    "task-return-with-note",
    "task-stop-run",
    "task-accept-step",
  ] {
    for by in [Answerer::Person, Answerer::Automation] {
      let mut f = if button == "task-accept-step" {
        Fixture::live_decision("Report unverified.", TaskView::Summary)
      } else {
        Fixture::new("decisions", "Check the empty states on screen", TaskView::Summary)
      };
      f.app.until("decision online", |app| {
        app.shows(button) && !app.shows("task-retry-offline")
      });
      if button == "task-return-with-note" {
        f.app.set_by_client("task-return-note", "Cover the empty project.");
      }
      if by == Answerer::Automation {
        f.refused_then_consent(button, "task-decision-person-only");
      }
      f.press(button, by);
      let db = f.db();
      let run = f.run;
      f.app.until(&format!("{button} by {by:?} stored"), |app| {
        assert!(
          !app.shows("task-retry-refused"),
          "{button} by {by:?}: {}",
          super::tools_window::text(app, "task-retry-refused")
        );
        db.events(run).expect("events").iter().any(|event| {
          matches!(event.kind(),
          RunEventKind::StuckDecided { by: recorded, .. } if *recorded == by)
        })
      });
      assert_eq!(f.stuck_by(), Some(by));
    }
  }
}

#[test]
fn person_decisions_composer_question_and_continuation_keep_the_decider() {
  for (scenario, title) in [
    ("task-states", "Sync contacts with HubSpot"),
    ("decisions", "Check the empty states on screen"),
  ] {
    for by in [Answerer::Person, Answerer::Automation] {
      let mut f = Fixture::new(scenario, title, TaskView::Conversation);
      f.app.until("composer", |app| app.shows("composer-input"));
      f.app.set_by_client("composer-input", "Keep the current behavior.");
      if by == Answerer::Automation {
        f.press("composer-send", by);
        f.app.until("refusal", |app| app.shows("composer-failed"));
        assert!(
          f.db()
            .messages(f.db().run(f.run).expect("run").expect("run").task())
            .expect("messages")
            .iter()
            .all(|message| message.body().as_str() != "Keep the current behavior.")
        );
        f.consent();
      }
      f.press("composer-send", by);
      let db = f.db();
      let task = db.run(f.run).expect("run").expect("run").task();
      f.app.until("answer stored", |_| {
        db.messages(task)
          .expect("messages")
          .iter()
          .any(|message| message.body().as_str() == "Keep the current behavior." && message.answerer() == Some(by))
      });
    }
  }
}

#[test]
fn person_decisions_approval_card_refuses_and_records_the_decider() {
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("cards", "Add serde to the webhook payloads", TaskView::Conversation);
    let button = f.card_button("cards-approval-", "-deny");
    let refused = format!("{}-refused", button.strip_suffix("-deny").expect("suffix"));
    if by == Answerer::Automation {
      f.refused_then_consent(&button, &refused);
      assert!(f.db().decided_tool_approvals(f.run).expect("decisions").is_empty());
    }
    f.press(&button, by);
    let db = f.db();
    let run = f.run;
    f.app.until("approval answer stored", |_| {
      db.decided_tool_approvals(run).expect("decisions").iter().any(|answer| {
        answer.decided_by.is_some_and(|recorded| match by {
          Answerer::Person => recorded.by_person(),
          Answerer::Automation => recorded.by_automation(),
        })
      })
    });
  }
}

#[test]
fn person_decisions_question_card_refuses_and_records_the_decider() {
  use orchester_core::question::QuestionState;
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("cards", "Pick where the docs button goes", TaskView::Conversation);
    let button = f.card_button("cards-choice-", "-send");
    let card = button.strip_suffix("-send").expect("suffix");
    let refused = format!("{card}-refused");
    f.app.click_by_person(&format!("{card}-option-0-1"));
    if by == Answerer::Automation {
      f.refused_then_consent(&button, &refused);
      assert!(
        f.db()
          .questions(f.run)
          .expect("questions")
          .iter()
          .any(|asked| asked.waits())
      );
    }
    f.press(&button, by);
    let db = f.db();
    let run = f.run;
    f.app.until("question answer stored", |_| {
      db.questions(run)
        .expect("questions")
        .iter()
        .any(|asked| matches!(asked.state, QuestionState::Answered { by: recorded, .. } if recorded == by))
    });
  }
}

#[test]
fn person_decisions_plan_card_refuses_and_records_the_decider() {
  for by in [Answerer::Person, Answerer::Automation] {
    let mut f = Fixture::new("task-states", "Rate-limit messages from GitHub", TaskView::Conversation);
    let button = f.card_button("cards-plan-", "-approve");
    let refused = format!("{}-refused", button.strip_suffix("-approve").expect("suffix"));
    if by == Answerer::Automation {
      f.refused_then_consent(&button, &refused);
    }
    f.press(&button, by);
    let db = f.db();
    let run = f.run;
    f.app.until("plan approval stored", |_| {
      db.plans(run)
        .expect("plans")
        .iter()
        .any(|plan| plan.review.as_ref().is_some_and(|review| review.by == by))
    });
    assert_eq!(f.plan_by(), Some(by));
  }
}

#[test]
fn person_decisions_domain_tools_record_automation() {
  use orchester_core::question::QuestionState;
  for tool in ["decide_approval", "answer_question", "decide_plan", "continue_run"] {
    let (scenario, title) = match tool {
      "decide_approval" => ("cards", "Add serde to the webhook payloads"),
      "answer_question" => ("cards", "Pick where the docs button goes"),
      "decide_plan" => ("task-states", "Rate-limit messages from GitHub"),
      _ => ("decisions", "Check the empty states on screen"),
    };
    let mut f = Fixture::new(scenario, title, TaskView::Conversation);
    let mut args = json!({"project":"Kontur","run":f.run.to_string()});
    match tool {
      "decide_approval" => {
        args = json!({
          "project":"Kontur",
          "approval":f.db().pending_tool_approvals(f.run).expect("requests")[0].id,
          "decision":"deny"
        });
      }
      "answer_question" => args["choices"] = json!(["In the top bar"]),
      "decide_plan" => {
        args["decision"] = json!("request_changes");
        args["note"] = json!("Keep the retry bounded.");
      }
      _ => args["choice"] = json!({"kind":"allow_retry"}),
    }
    assert_eq!(f.app.refused(tool, &args)["code"], "automation_not_allowed");
    f.consent();
    f.app.ok(tool, &args);
    let db = f.db();
    match tool {
      "decide_approval" => assert!(
        db.decided_tool_approvals(f.run)
          .expect("decisions")
          .iter()
          .any(|answer| answer.decided_by.is_some_and(|by| by.by_automation()))
      ),
      "answer_question" => assert!(db.questions(f.run).expect("questions").iter().any(|asked| matches!(
        asked.state,
        QuestionState::Answered {
          by: Answerer::Automation,
          ..
        }
      ))),
      "decide_plan" => assert_eq!(f.plan_by(), Some(Answerer::Automation)),
      _ => assert_eq!(f.stuck_by(), Some(Answerer::Automation)),
    }
  }
}

#[test]
fn person_decisions_native_orchestrator_acceptance_keeps_the_person() {
  use orchester_core::{orchestrator::EntryKind, question::QuestionState};
  use orchester_workspace::{app_mcp, orchestrator};
  let mut f = Fixture::new("cards", "Pick where the docs button goes", TaskView::Conversation);
  let mut db = f.db();
  let scope = super::orchestrator_chat::chat(&f.app, &mut db);
  let answer = json!({"project":"Kontur","run":f.run.to_string(),"choices":["In the top bar"]});
  let proposed = super::orchestrator_chat::chat_call(&mut f.app, &scope, "answer_question", answer).expect("proposed");
  assert_eq!(proposed["waits_for_person"], true);
  let seq = db
    .chat_entries(scope.chat)
    .expect("entries")
    .into_iter()
    .find_map(|entry| matches!(entry.kind, EntryKind::Action(_)).then_some(entry.seq))
    .expect("action");
  let s = f.app.services();
  assert!(!s.workspace.automation_approval_consent());
  let services = app_mcp::Services::new(
    s.workspace.clone(),
    s.runners.clone(),
    s.servers.clone(),
    s.programs.clone(),
    (s.changes.clone(), s.activity.clone(), s.jobs.clone()),
  );
  let runners = s.runners.clone();
  let decided = f.app._runtime.spawn(async move {
    let translate = app_mcp::Translate::new(|message| message.key.to_owned());
    orchestrator::proposal::decide(
      (&services, &translate, &runners),
      &scope,
      seq,
      orchestrator::proposal::Decision::Accept { widens: Vec::new() },
    )
    .await
  });
  f.app.until("native acceptance", |_| decided.is_finished());
  f.app
    ._runtime
    .block_on(decided)
    .expect("task")
    .expect("accepted without an automation grant");
  assert!(db.questions(f.run).expect("questions").iter().any(|asked| matches!(
    asked.state,
    QuestionState::Answered {
      by: Answerer::Person,
      ..
    }
  )));
}

#[test]
fn person_decisions_domain_tools_share_consent_for_every_kind() {
  let (mut app, _) = Served::seeded("task-states");
  for (tool, args) in [
    (
      "decide_approval",
      json!({"project":"Kontur","approval":"unknown","decision":"deny"}),
    ),
    (
      "answer_question",
      json!({"project":"Kontur","run":"999","text":"Keep it."}),
    ),
    (
      "decide_plan",
      json!({"project":"Kontur","run":"999","decision":"request_changes","note":"Keep it."}),
    ),
    (
      "continue_run",
      json!({"project":"Kontur","run":"999","choice":{"kind":"allow_retry"}}),
    ),
  ] {
    assert_eq!(app.refused(tool, &args)["code"], "automation_not_allowed", "{tool}");
    app
      .services()
      .workspace
      .set_automation_answers_approvals(true)
      .expect("consent");
    assert_ne!(app.refused(tool, &args)["code"], "automation_not_allowed", "{tool}");
    app
      .services()
      .workspace
      .set_automation_answers_approvals(false)
      .expect("revoke");
    assert_eq!(app.refused(tool, &args)["code"], "automation_not_allowed", "{tool}");
  }
}
