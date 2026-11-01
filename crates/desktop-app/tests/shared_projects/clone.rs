//! *Clone it here* on *Link a folder* (ORC-584; 06 `go2aM`, 07 `phoYc`, Row 3b): the panel in place of the `git clone`
//! command, *Choose…* filling where the checkout goes, a folder with files refused before any console opens, and each
//! way a clone ends: cloned and chosen in the folder field but linked only when the person presses, still running,
//! failed with what Git left kept, closed, Git missing, Git done without a checkout. Only a person opens the console.
//! The console is the test window's stand-in (`ConsoleScript`); the real one is `orchester-exec`'s `clone_console`.

use std::{fs, path::PathBuf};

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::Route;

use super::{
  FOLDER, HINT, SUBMIT, acme, at_link, has, shows,
  support::{Window, stand_ins::ConsoleScript},
};

const INTO: &str = "join-team-link-clone-into";
const START: &str = "join-team-link-clone-start";
const URL: &str = "https://github.com/acme/kontur.git";

fn value(window: &mut Window, id: &str) -> String {
  window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_text_input())
    .unwrap_or_else(|| panic!("{id} is a text input"))
    .value()
}

/// The folder `code` in the window's data, where clones go.
fn code(window: &Window) -> PathBuf {
  let code = window.data.dir.path().join("code");
  fs::create_dir_all(&code).expect("the folder for clones");
  code
}

/// Types `destination` into *Clone into* and waits for what Git would do there.
fn clone_into(window: &mut Window, destination: &std::path::Path) {
  window.set(INTO, &destination.display().to_string());
  window.until("the destination to be checked", |window| {
    has(window, "join-team-link-clone-target") || has(window, "join-team-link-clone-error")
  });
}

#[test]
fn shared_projects_clone_panel_replaces_the_command() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  shows(&mut window, "join-team-link-clone");
  assert_eq!(
    window.text("join-team-link-clone-title"),
    "No checkout yet? Clone it here"
  );
  assert!(window.text("join-team-link-clone-sub").contains(HINT));
  assert!(!has(&mut window, "join-team-link-clone-command"), "no command to copy");
  assert!(
    window
      .text("join_team-lead")
      .ends_with("use a checkout you have, or clone it here.")
  );
  assert_eq!(window.text(START), "Clone repository…");
  window.press(START);
  window.pass();
  assert!(stand_ins.consoles().is_empty(), "nothing to clone into yet");
}

#[test]
fn shared_projects_clone_choose_fills_the_destination() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  let code = code(&window);
  stand_ins.choose(Some(code.clone()));
  window.press("join-team-link-clone-choose");
  let destination = code.join("kontur").display().to_string();
  window.until("the destination to be filled", |window| {
    value(window, INTO) == destination
  });
  shows(&mut window, "join-team-link-clone-target");
  assert_eq!(
    window.text("join-team-link-clone-target-text"),
    format!("Creates the folder kontur in {}", code.display())
  );
}

#[test]
fn shared_projects_clone_an_occupied_folder_is_refused_before_any_console() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Clones);
  let occupied = code(&window).join("kontur");
  fs::create_dir_all(&occupied).expect("a folder");
  fs::write(occupied.join(".keep"), "mine").expect("a hidden file");
  clone_into(&mut window, &occupied);
  assert_eq!(
    window.text("join-team-link-clone-error"),
    "This folder already has files. Choose an empty folder or a new name: Orchester never clones over files."
  );
  window.press(START);
  window.pass();
  assert!(stand_ins.consoles().is_empty(), "no console opened");
  assert_eq!(fs::read_to_string(occupied.join(".keep")).expect("kept"), "mine");
}

#[test]
fn shared_projects_clone_chooses_the_checkout_and_links_only_when_pressed() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Clones);
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-done");
  let path = destination.display().to_string();
  assert_eq!(
    window.text("join-team-link-clone-done-text"),
    format!("Cloned into {path}. It's chosen above: link it with the button below.")
  );
  assert_eq!(value(&mut window, FOLDER), path, "the folder field holds the clone");
  shows(&mut window, "join-team-link-check");
  assert_eq!(
    window.text("join-team-link-check-text"),
    format!("Git repository · origin {HINT} matches the project")
  );
  // Clone into is checked again as the clone ends, before Git checks the folder: the new checkout is no error there.
  window.pass();
  assert!(
    !has(&mut window, "join-team-link-clone-error"),
    "the new checkout is not refused as a folder to clone into"
  );
  assert!(!has(&mut window, "join-team-link-clone-target"));
  let jobs = stand_ins.consoles();
  assert_eq!(jobs.len(), 1);
  assert_eq!((jobs[0].url.as_str(), &jobs[0].destination), (URL, &destination));
  assert_eq!(jobs[0].words.title(), "Orchester · cloning Kontur");
  assert!(
    !destination.join(".orchester").exists(),
    "nothing is linked before the press"
  );

  window.press(SUBMIT);
  window.until("the project to open", |window| {
    matches!(window.navigation().route_now(), Route::Project(..))
  });
  assert!(destination.join(".orchester").exists(), "the clone was linked");
}

#[test]
fn shared_projects_clone_shows_the_console_while_it_runs() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Holds(Box::new(ConsoleScript::Clones)));
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-waiting");
  assert_eq!(
    window.text("join-team-link-clone-waiting-title"),
    "Git is cloning in a console window"
  );
  assert_eq!(window.text(START), "Cloning in a console…");
  window.press(START);
  stand_ins.release_console();
  shows(&mut window, "join-team-link-clone-done");
  assert!(!has(&mut window, "join-team-link-clone-waiting"));
  assert_eq!(stand_ins.consoles().len(), 1, "one console");
}

#[test]
fn shared_projects_clone_follows_the_console_after_the_page_was_left() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Holds(Box::new(ConsoleScript::Clones)));
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-waiting");
  window.press("join-team-link-back");
  shows(&mut window, "join-team-projects-continue");
  window.press("join-team-projects-continue");
  shows(&mut window, "join-team-link-clone-waiting");
  stand_ins.release_console();
  shows(&mut window, "join-team-link-clone-done");
  assert_eq!(value(&mut window, FOLDER), destination.display().to_string());
  assert_eq!(stand_ins.consoles().len(), 1, "the same console");
}

#[test]
fn shared_projects_clone_a_failure_says_why_and_keeps_the_folder() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::WritesThen(Box::new(ConsoleScript::Exits(128))));
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-failed");
  assert_eq!(
    window.text("join-team-link-clone-failed-title"),
    "Git stopped before the clone finished (code 128)"
  );
  assert!(window.text("join-team-link-clone-failed-body").contains(HINT));
  shows(&mut window, "join-team-link-clone-left");
  assert!(
    window
      .text("join-team-link-clone-left-text")
      .starts_with(&format!("Git left the folder {}.", destination.display()))
  );
  assert!(destination.join(".git").join("HEAD").exists(), "what Git left is kept");
  assert_eq!(value(&mut window, FOLDER), "", "nothing was chosen");
  shows(&mut window, "join-team-link-clone-error");
  window.press(START);
  window.pass();
  assert_eq!(stand_ins.consoles().len(), 1, "no console over what Git left");

  stand_ins.console(ConsoleScript::Clones);
  let another = code(&window).join("kontur-2");
  clone_into(&mut window, &another);
  window.press(START);
  shows(&mut window, "join-team-link-clone-done");
}

#[test]
fn shared_projects_clone_a_closed_console_can_be_tried_again() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Closes);
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-closed");
  assert_eq!(
    window.text("join-team-link-clone-closed"),
    "The console closed before the clone finished Nothing was linked."
  );
  assert!(!has(&mut window, "join-team-link-clone-left"), "Git left nothing");

  stand_ins.console(ConsoleScript::Clones);
  window.press(START);
  shows(&mut window, "join-team-link-clone-done");
  assert!(!has(&mut window, "join-team-link-clone-closed"));
}

#[test]
fn shared_projects_clone_without_git_says_so() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::GitMissing);
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-git-missing");
  assert_eq!(
    window.text("join-team-link-clone-git-missing-title"),
    "Git isn't installed"
  );
}

#[test]
fn shared_projects_clone_git_done_without_a_checkout_says_so() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::WritesThen(Box::new(ConsoleScript::Exits(0))));
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  window.press(START);
  shows(&mut window, "join-team-link-clone-not-checkout");
  shows(&mut window, "join-team-link-clone-left");
  assert_eq!(value(&mut window, FOLDER), "", "nothing was chosen");
}

#[test]
fn shared_projects_clone_only_a_person_opens_the_console() {
  let server = acme();
  let (mut window, stand_ins) = at_link(&server);
  stand_ins.console(ConsoleScript::Clones);
  let destination = code(&window).join("kontur");
  clone_into(&mut window, &destination);
  with_synthetic_input(|| window.press(START));
  shows(&mut window, &format!("{START}-person-only"));
  assert!(stand_ins.consoles().is_empty(), "no console opened");
}
