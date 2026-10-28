//! A decision card while its facts are read: a merge's facts come from the read, so the card keeps their room from
//! its first frame, shows their bones after `SKELETON_DELAY` and does not change its height when they arrive; git's
//! work keeps its facts in its proposal and shows them on its first frame.

use std::{
  collections::BTreeMap,
  fs,
  path::Path,
  process::Command,
  time::{Duration, Instant},
};

use orchester_app::{
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::SKELETON_DELAY,
};
use orchester_core::{
  id::IdempotencyKey,
  orchestrator::{
    ActionOutcome, ChatAction,
    change::{Change, ProposedChange},
    git::{GitChange, GitOperation, GitPlace, GitProposal, ResetMode},
  },
  resource::{AgentProfile, Pipeline, StepKind},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, Run, RunSnapshot, RunStatus},
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};

use super::{
  harness::{Orchestrator, Setup, git},
  lines::with_action,
};

/// What git says to `args` in `repo`, trimmed.
fn read_git(repo: &Path, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(output.status.success(), "git {args:?}");
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A succeeded run of a new task on Delivery whose branch commits a file over `main`, ready to merge; the run and its
/// task's key.
fn ready_to_merge(screen: &Orchestrator) -> (Run, String) {
  let delivery = screen.delivery();
  let key = screen.draft_on(delivery, "Merge me");
  let repo = screen.root.as_path();
  let base = read_git(repo, &["rev-parse", "HEAD"]);
  let branch = format!("orchester/{}", key.to_lowercase());
  git(repo, &["checkout", "--quiet", "-b", &branch]);
  fs::write(repo.join("work.txt"), "work\n").expect("the run's work");
  git(repo, &["add", "work.txt"]);
  git(repo, &["commit", "--quiet", "-m", "work"]);
  let checkpoint = read_git(repo, &["rev-parse", "HEAD"]);
  git(repo, &["checkout", "--quiet", "main"]);

  let mut db = screen.db();
  let task = db
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .expect("the task");
  let version = db
    .latest_version::<Pipeline>(delivery)
    .expect("read the pipeline")
    .expect("published");
  let StepKind::Agent {
    profile: Some(builder), ..
  } = version.body().steps()[0].kind
  else {
    panic!("the step has a profile");
  };
  let profile = db
    .latest_version::<AgentProfile>(builder)
    .expect("read the profile")
    .expect("published");
  let profiles = BTreeMap::from([(builder, profile)]);
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &BTreeMap::new(),
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(delivery, &version, published).expect("snapshot");
  let new = NewRun {
    task: task.id(),
    snapshot,
    branch: BranchName::new(&branch).expect("branch"),
    base: Revision::new(&base).expect("revision"),
    base_branch: Some(BranchName::new("main").expect("branch")),
    budget: None,
    max_duration: None,
  };
  let now = Timestamp::now();
  let mut run = db.create_run(IdempotencyKey::generate(), new, now).expect("run");
  let events = run.start(now).expect("started");
  db.append_events(&run, &events).expect("kept");
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  // Each step's events are kept before the next is taken: the store checks the last one against the run.
  let events = run.start_step(execution, now).expect("step started");
  db.append_events(&run, &events).expect("kept");
  let events = run
    .record_checkpoint(execution, Revision::new(&checkpoint).expect("revision"), now)
    .expect("checkpoint");
  db.append_events(&run, &events).expect("kept");
  let events = run.complete_step(execution, now).expect("step passed");
  db.append_events(&run, &events).expect("kept");
  assert_eq!(*run.status(), RunStatus::Succeeded);
  (run, key)
}

/// Opens `chat` and lays it out, without delivering what the window's futures finished, until `card` shows: its
/// facts' read has not been delivered yet.
fn open_unread(screen: &mut Orchestrator, chat: OrchestratorChat, card: &str) {
  screen
    .window
    .navigation()
    .go(Route::Project(screen.project, ProjectPage::Orchestrator(chat)));
  let deadline = Instant::now() + Duration::from_secs(30);
  loop {
    // The conversation's own reads are delivered; the card's, started by the pass that first shows it, are not.
    screen.window.tree.tick_futures();
    screen.window.pass();
    if screen.exists(card) {
      return;
    }
    assert!(Instant::now() < deadline, "timed out waiting for {card}");
    std::thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn orchestrator_merge_card_keeps_its_height_while_its_facts_read() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let (run, key) = ready_to_merge(&screen);
  let merge = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Merge {
        run: run.id(),
        number: run.number().get(),
        checkpoint: run
          .last_checkpoint()
          .map(|checkpoint| checkpoint.revision().to_string()),
        into: Some("main".to_owned()),
      },
    })),
    ..ChatAction::new("merge_run", Some(key.clone()), ActionOutcome::Waits)
  };
  let (chat, card) = with_action(&screen, "Merge it?", &merge);
  open_unread(&mut screen, chat, &card);
  let (reading, bone) = (format!("{card}-facts-reading"), format!("{card}-facts-bone-2"));
  assert!(
    screen.exists(&reading) && !screen.exists(&format!("{card}-facts")),
    "the facts keep their room while they read"
  );
  assert!(
    !screen.exists(&bone),
    "no bones before the delay: a fast read never flashes them"
  );
  let waiting = screen.bounds(&card).height;

  screen
    .window
    .tree
    .tick_timers_at(Instant::now() + SKELETON_DELAY + Duration::from_millis(50));
  screen.window.pass();
  assert!(screen.exists(&bone), "the bones show after the delay");
  let skeleton = screen.bounds(&card).height;
  assert!(
    (skeleton - waiting).abs() < 0.5,
    "the bones keep the card's height: {skeleton} vs {waiting}"
  );

  screen.until("the merge's facts", |screen| screen.exists(&format!("{card}-fact-2")));
  let facts = screen.text(&format!("{card}-facts"));
  assert!(
    facts.contains(&format!("orchester/{}", key.to_lowercase())) && facts.contains("1 file"),
    "the run, its branch and its changes: {facts}"
  );
  let read = screen.bounds(&card).height;
  assert!(
    (read - waiting).abs() < 0.5,
    "the card does not grow when its facts arrive: {read} vs {waiting}"
  );
}

#[test]
fn orchestrator_git_card_shows_its_facts_before_anything_is_read() {
  let mut screen = Orchestrator::new(Setup::Ready);
  let reset = ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: String::new(),
      change: Change::Git(Box::new(GitChange {
        place: GitPlace::Checkout,
        operation: GitOperation::Propose(GitProposal::Reset {
          to: "HEAD~1".to_owned(),
          mode: ResetMode::Hard,
        }),
        commands: vec!["git reset --hard HEAD~1".to_owned()],
        said: None,
        basis: String::new(),
        loses: vec!["1 commit".to_owned()],
      })),
    })),
    ..ChatAction::new("git_reset", None, ActionOutcome::Waits)
  };
  let (chat, card) = with_action(&screen, "Reset?", &reset);
  open_unread(&mut screen, chat, &card);
  assert!(
    screen.exists(&format!("{card}-facts")),
    "git's work shows where it runs on its first frame"
  );
  assert!(
    !screen.exists(&format!("{card}-facts-reading")),
    "it keeps no room for a read"
  );
}
