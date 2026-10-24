//! The chat's git tools in the window (decision 9 of `docs/design/orchestrator-toolset.md`): a write is refused while
//! *Let the Orchestrator write with git* is off and runs once the person turned it on, each kept as a card with what
//! ran; what can lose work waits as a card that only a person applies: automation's click is refused (`person_only`)
//! and changes nothing, the person's resets the branch as the card showed.

use std::{fs, process::Command};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::OrchestratorChat;
use orchester_core::orchestrator::ActionOutcome;
use orchester_workspace::workspace::OrchestratorConsent;

use super::{
  harness::{Orchestrator, Setup, git},
  proposal::{outcomes, proposal_card},
};

/// `git <args>` in the project's folder; its output.
fn ask(screen: &Orchestrator, args: &[&str]) -> String {
  let output = Command::new("git")
    .arg("-C")
    .arg(&screen.root)
    .args(args)
    .output()
    .expect("git runs");
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Sends `text` and waits for its turn's action to be `outcome` and the turn to end; the card's id.
fn acted(screen: &mut Orchestrator, text: &str, outcome: fn(&ActionOutcome) -> bool) -> String {
  let before = outcomes(screen).len();
  screen.send(text);
  screen.until("the action", |screen| outcomes(screen).get(before).is_some_and(outcome));
  screen.until("the turn's end", |screen| !screen.exists("orchestrator-thinking"));
  let card = proposal_card(screen);
  screen.until("the card", |screen| screen.exists(&format!("{card}-title")));
  card
}

#[test]
fn orchestrator_git_write_waits_for_the_switch_and_shows_what_ran() {
  let mut screen = Orchestrator::new(Setup::Ready);
  screen.go(OrchestratorChat::New);
  fs::write(screen.root.join("notes.txt"), "from the chat\n").expect("a file");
  let commit = r#"Call git_commit {"project":"Kontur","message":"Add notes","paths":["notes.txt"]}"#;
  let card = acted(
    &mut screen,
    commit,
    |outcome| matches!(outcome, ActionOutcome::Refused { code, .. } if code == "write_off"),
  );
  assert!(
    screen
      .text(&format!("{card}-effect"))
      .contains("Let the Orchestrator write with git"),
    "the card says why"
  );
  assert_eq!(
    ask(&screen, &["log", "-1", "--format=%s"]),
    "initial",
    "nothing was committed"
  );

  screen.let_orchestrator(OrchestratorConsent::WriteGit, true);
  let card = acted(&mut screen, commit, |outcome| *outcome == ActionOutcome::Done);
  assert_eq!(screen.text(&format!("{card}-title")), "Ran git commit");
  let commands = screen.text(&format!("{card}-commands"));
  assert!(
    commands.contains("git add -A -- notes.txt") && commands.contains("git commit -F - -- notes.txt"),
    "{commands}"
  );
  assert!(
    screen.text(&format!("{card}-effect")).contains("Add notes"),
    "what git said"
  );
  assert_eq!(ask(&screen, &["log", "-1", "--format=%s"]), "Add notes");
}

#[test]
fn orchestrator_git_proposal_is_applied_only_by_a_person() {
  let mut screen = Orchestrator::new(Setup::Ready);
  fs::write(screen.root.join("later.txt"), "later\n").expect("a file");
  git(&screen.root, &["add", "later.txt"]);
  git(&screen.root, &["commit", "--quiet", "-m", "Later work"]);
  let before = ask(&screen, &["rev-parse", "HEAD"]);
  screen.go(OrchestratorChat::New);
  let reset = r#"Call git_propose {"project":"Kontur","operation":{"kind":"reset","to":"HEAD~1","mode":"hard"}}"#;
  let card = acted(&mut screen, reset, |outcome| *outcome == ActionOutcome::Waits);
  screen.until("its Apply", |screen| screen.exists(&format!("{card}-start")));
  assert_eq!(screen.text(&format!("{card}-title")), "Run git reset --hard?");
  assert!(
    screen.text(&format!("{card}-loses")).contains("Later work"),
    "what it loses"
  );

  let (start, person_only) = (format!("{card}-start"), format!("{card}-person-only"));
  screen.until("the refusal", |screen| {
    with_synthetic_input(|| screen.click(&start));
    screen.exists(&person_only)
  });
  assert_eq!(
    outcomes(&screen)[0],
    ActionOutcome::Waits,
    "automation's click applies nothing"
  );
  assert_eq!(ask(&screen, &["rev-parse", "HEAD"]), before);

  screen.click(&start);
  screen.until("the reset", |screen| outcomes(screen)[0] == ActionOutcome::Applied);
  assert_ne!(
    ask(&screen, &["rev-parse", "HEAD"]),
    before,
    "the person's Apply reset the branch"
  );
  assert!(!screen.root.join("later.txt").exists());
}
