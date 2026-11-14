//! *While you were away* on the Overview (`W01 · Overview · parked for you and decided without you`, OA-23): a task
//! the Orchestrator parked shows with what is needed and its words, and **Give it back** ends the parking; a decision
//! it made by itself shows with its reason, and **Seen** takes it off the list. With neither, nothing of it shows.

use orchester_app::routes::OverviewTab;
use orchester_core::{
  id::ChatId,
  orchestrator::{
    decision::{NewDecision, Reason},
    parking::{Needs, Parking, Words},
  },
  store::TaskStore,
  time::Timestamp,
};
use orchester_store::{DecisionsRead, ProjectDb};

use crate::harness::Seeded;

#[test]
fn overview_away_shows_what_was_parked_and_decided_and_settles_each_by_a_press() {
  let mut kept = None;
  let seeded = Seeded::prepared("tasks", false, |repo| {
    let mut db = ProjectDb::open(repo).expect("the project database opens");
    let task = db.tasks().expect("tasks").remove(0);
    let chat = ChatId::generate();
    db.park_task(&Parking {
      task: task.id(),
      run: None,
      needs: Needs::Approval,
      why: Words::new("The Orchestrator may not allow a push outside the project.").expect("words"),
      to_do: Words::new("Allow or deny it on the request's card.").expect("words"),
      by: chat,
      at: Timestamp::now(),
    })
    .expect("parked");
    let number = db
      .keep_decision(&NewDecision {
        chat,
        tool: "continue_run".to_owned(),
        task: Some(task.key().to_string()),
        run: None,
        reason: Reason::new("Run checks failed on a test the change did not touch.").expect("a reason"),
        undo: None,
        at: Timestamp::now(),
      })
      .expect("kept");
    kept = Some((task.key().to_string(), number));
  });
  let (key, number) = kept.expect("the task and its decision");
  let mut seeded = Seeded::shown(seeded, OverviewTab::Running);
  seeded.until("what was parked", |seeded| seeded.shows("overview-parked"));
  assert_eq!(seeded.text("overview-parked-title"), "1 parked for you");
  assert_eq!(
    seeded.text(&format!("overview-parked-{key}-what")),
    format!("{key} parked — an approval is needed")
  );
  let words = seeded.text(&format!("overview-parked-{key}-words"));
  assert!(
    words.contains("may not allow a push") && words.contains("Allow or deny it"),
    "{words}"
  );
  assert_eq!(seeded.text("overview-decided-title"), "1 decided without you");
  assert_eq!(
    seeded.text(&format!("overview-decision-{number}-what")),
    format!("Continued {key}")
  );
  assert!(
    seeded
      .text(&format!("overview-decision-{number}-reason"))
      .contains("did not touch")
  );

  seeded.click(&format!("overview-decision-{number}-seen"));
  seeded.until("the decision to go", |seeded| !seeded.shows("overview-decided"));
  let db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  let open = DecisionsRead {
    open_only: true,
    ..DecisionsRead::default()
  };
  assert!(db.decisions(&open).expect("read").is_empty(), "marked seen");

  seeded.click(&format!("overview-parked-{key}-give-back"));
  seeded.until("the parking to end", |seeded| !seeded.shows("overview-parked"));
  assert!(db.parked_tasks().expect("read").is_empty(), "given back");
}

#[test]
fn overview_away_shows_nothing_when_nothing_is_parked_or_decided() {
  let mut seeded = Seeded::overview("tasks", OverviewTab::Running);
  seeded.until("the page", |seeded| seeded.shows("overview-away"));
  assert!(!seeded.shows("overview-parked") && !seeded.shows("overview-decided"));
}
