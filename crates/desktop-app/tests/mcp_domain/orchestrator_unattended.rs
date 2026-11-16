//! OA-26: a module led to the end with nobody at the computer, against the real runner and the fake Codex
//! (`live`): a project set up through the tools, a module of three tasks, and a conversation that leads the module
//! to the end. The test plays the Orchestrator's part and nothing else: in turns nobody asked for it reads where
//! the module stands (`get_standing`) and calls the chat's own tools for what it reads, as the instructions of a
//! leading turn say to: it starts what can start, merges what every step passed, runs a failed task again with
//! its reason until its ceiling refuses, and parks what only a person can settle. No person's press and no answer
//! of a person is given.
//!
//! | Task | Its agent | Where it ends |
//! | --- | --- | --- |
//! | Greet | works | merged into main |
//! | Sign | reports failure every time | run three times (the ceiling), then parked |
//! | Publish | asks to push, which only a person may allow | parked, its run still waiting |
//!
//! It proves the app's side: the standing, the ceilings, the parkings, the decisions with their reasons and the
//! stuck target. A request that waits for a person reads as `working` in the standing, so the turn reads the
//! requests themselves, as the turn of a request's event does. A model's judgement, the follower's own turns (no
//! Orchestrator is set up here, so its events stay kept), a turn that fails and is tried again, the daily budget, a
//! stall and a team project are not in it; they have their own tests (`orchestrator_led_waits`, `orchestrator_held`,
//! `orchestrator_stalls`, `team_mcp`). `cargo test -p orchester-app --features test-support --test mcp_domain
//! orchestrator_unattended`.

use orchester_core::{
  id::ModuleId,
  orchestrator::{Ceilings, Subscription, WatchMode, WatchTarget, parking::Needs},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb};
use orchester_workspace::orchestrator::tools::ChatScope;
use serde_json::{Value, json};

use super::{
  Served,
  live::{git, served, set_up},
  orchestrator_chat::{allow, chat, chat_call},
};

const MODULE: &str = "Webhooks v2";

/// Creates a task of the module with `brief`, which tells the fake agent what to do; its key.
fn task(served: &mut Served, title: &str, brief: &str) -> String {
  let created = served.ok(
    "create_task",
    &json!({
      "project": "Kontur",
      "title": title,
      "brief": brief,
      "criteria": ["It is done."],
      "pipeline": "Delivery",
      "module": MODULE,
    }),
  );
  created["task"]["key"].as_str().expect("a key").to_owned()
}

/// Where the led module stands, as the chat reads it.
fn standing(served: &mut Served, own: &ChatScope) -> Value {
  let read = chat_call(served, own, "get_standing", json!({ "project": "Kontur" })).expect("the standing");
  read["subscriptions"][0].clone()
}

/// The id of the latest run of the task with `key`.
fn latest_run(db: &ProjectDb, key: &str) -> String {
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let run = db
    .runs_of_task(task.id())
    .expect("runs")
    .pop()
    .expect("a run")
    .expect("a readable run");
  run.id().to_string()
}

fn park(served: &mut Served, own: &ChatScope, key: &str, (needs, why): (&str, &str)) {
  let parked = chat_call(
    served,
    own,
    "park_task",
    json!({
      "project": "Kontur",
      "task": key,
      "needs": needs,
      "why": why,
      "to_do": "Look at the task's conversation and settle it, then give the task back.",
    }),
  );
  parked.unwrap_or_else(|error| panic!("{key} is parked: {error:?}"));
}

/// One turn nobody asked for: acts on each task of `led` as a leading turn does. Whether it did anything.
fn turn(served: &mut Served, own: &ChatScope, db: &ProjectDb, led: &Value) -> bool {
  // A request only a person may answer comes to the chat as its own event; the module's standing reads its run as
  // still working. The turn of that event reads the request, as here.
  let asked = served.ok("list_approvals", &json!({ "project": "Kontur" }));
  let asks: Vec<&str> = asked["approvals"]
    .as_array()
    .expect("approvals")
    .iter()
    .filter_map(|approval| approval["task"].as_str())
    .collect();
  let mut acted = false;
  for task in led["tasks"].as_array().expect("tasks") {
    let key = task["task"].as_str().expect("a key").to_owned();
    let start = json!({ "project": "Kontur", "key": key });
    match task["standing"].as_str().expect("a standing") {
      "ready_to_start" => {
        chat_call(served, own, "run_task", start).unwrap_or_else(|error| panic!("{key} starts: {error:?}"));
      }
      "to_merge" => {
        let run = json!({ "project": "Kontur", "run": latest_run(db, &key) });
        // The app may have merged it by itself meanwhile, under the same rule.
        if let Err(error) = chat_call(served, own, "merge_run", run)
          && error.code() != "already_merged"
        {
          panic!("{key} merges: {error:?}");
        }
      }
      "failed" => {
        let mut again = start;
        again["reason"] = json!("Its step reported a failure the brief does not explain; one more run is cheap.");
        if let Err(error) = chat_call(served, own, "run_task", again) {
          assert_eq!(error.code(), "ceiling_reached", "{key}: {error:?}");
          park(
            served,
            own,
            &key,
            ("ceiling", "It failed in every run its ceiling allows."),
          );
        }
      }
      "working" | "waits_for_person" if asks.contains(&key.as_str()) => park(
        served,
        own,
        &key,
        ("approval", "Its agent asks for a push, which only a person may allow."),
      ),
      _ => continue,
    }
    acted = true;
  }
  acted
}

/// The project left to the Orchestrator: its module's three tasks and the conversation that leads the module to
/// the end, in turns nobody asked for.
struct Left {
  served: Served,
  repo: std::path::PathBuf,
  db: ProjectDb,
  own: ChatScope,
  /// The keys of *Greet*, *Sign* and *Publish*.
  keys: [String; 3],
}

fn leave() -> Left {
  let (mut served, repo, fake) = served(false);
  set_up(&mut served, &repo, &fake);
  let module = served.ok("create_module", &json!({ "project": "Kontur", "name": MODULE }));
  let module: ModuleId = module["module"]["id"]
    .as_str()
    .expect("an id")
    .parse()
    .expect("a module id");
  let keys = [
    task(&mut served, "Greet", "Write own file."),
    task(&mut served, "Sign", "Write own file. Report failure."),
    task(
      &mut served,
      "Publish",
      "Write own file. Escalate `git push origin main`.",
    ),
  ];
  let mut db = ProjectDb::open(&repo).expect("the project's data");
  allow(&served, &repo);
  let asked = chat(&served, &mut db);
  let lead = WatchMode::Lead {
    ceilings: Ceilings::default(),
  };
  let leading = Subscription::new(asked.chat, WatchTarget::Module(module), "", Timestamp::now()).in_mode(lead);
  db.take_over_subscription(&leading)
    .expect("the person has the module led to the end");
  let own = ChatScope {
    on_its_own: true,
    ..asked
  };
  Left {
    served,
    repo,
    db,
    own,
    keys,
  }
}

/// Turn after turn, with nobody asked and nobody answering, until nothing more can move; where the module stands
/// then.
fn lead(left: &mut Left) -> Value {
  let own = left.own.clone();
  let mut led = standing(&mut left.served, &own);
  for _ in 0..40 {
    // A task past its ceiling already reads the module as stuck: the turn still parks it, with what is needed.
    if turn(&mut left.served, &own, &left.db, &led) {
      led = standing(&mut left.served, &own);
      continue;
    }
    if matches!(led["standing"].as_str(), Some("stuck" | "done")) {
      break;
    }
    let before = led.clone();
    left.served.until("the module's standing to move", |served| {
      standing(served, &own) != before
    });
    led = standing(&mut left.served, &own);
  }
  led
}

#[test]
fn orchestrator_unattended_leads_a_module_until_every_task_is_merged_or_parked() {
  let mut left = leave();
  let led = lead(&mut left);
  let [greet, sign, publish] = left.keys.clone();
  let (db, repo) = (&left.db, left.repo.clone());

  assert_eq!(led["standing"], "stuck", "all that is left is the person's: {led}");
  let stands = |key: &str| {
    let tasks = led["tasks"].as_array().expect("tasks");
    let task = tasks.iter().find(|task| task["task"] == key).expect("the task");
    task["standing"].as_str().expect("a standing").to_owned()
  };
  assert_eq!(stands(&greet), "done", "{led}");
  assert_eq!(stands(&sign), "parked", "{led}");
  assert_eq!(stands(&publish), "parked", "{led}");
  assert_eq!(
    git(&repo, &["show", "main:implement.txt"]).trim(),
    "hello",
    "main holds the merged task's work"
  );

  // The failing task ran as often as its ceiling allows, and no more.
  let tasks = db.tasks().expect("tasks");
  let failing = tasks
    .iter()
    .find(|task| task.key().to_string() == sign)
    .expect("the task");
  let runs = db.runs_of_task(failing.id()).expect("runs").len();
  assert_eq!(runs, usize::from(Ceilings::default().runs_of_task()));

  // Both parkings say what is needed.
  let mut needs: Vec<Needs> = db
    .parked_tasks()
    .expect("parked")
    .iter()
    .map(|parking| parking.needs)
    .collect();
  needs.sort_by_key(|needs| needs.name());
  assert_eq!(needs, [Needs::Approval, Needs::Ceiling]);

  // Every judgement is a decision with its reason: each further run of the failing task and each parking.
  let decisions = db.decisions(&DecisionsRead::default()).expect("decisions");
  assert!(
    decisions
      .iter()
      .all(|decision| !decision.made.reason.as_str().is_empty()),
    "{decisions:?}"
  );
  let made = |tool: &str| decisions.iter().filter(|decision| decision.made.tool == tool).count();
  assert_eq!(made("park_task"), 2, "{decisions:?}");
  assert_eq!(made("run_task"), runs - 1, "{decisions:?}");

  // With the switch off the chat starts nothing by itself.
  left
    .served
    .services()
    .workspace
    .set_orchestrator_drives(&repo, false)
    .expect("the person turned it off");
  let start = json!({ "project": "Kontur", "key": greet });
  let own = left.own.clone();
  assert!(
    chat_call(&mut left.served, &own, "run_task", start).is_err(),
    "nothing starts with the switch off"
  );
}
