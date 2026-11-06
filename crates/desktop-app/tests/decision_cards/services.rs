//! The cards a run's services put in its Conversation, on the QA seed's `service-cards` scenario (ORC-275): a service
//! that crashed (`FwHrv`), a start that failed because Docker did not answer (10a) or a service was not ready in time
//! (10c), a start that waits for *Services at once* slots (10d), and a start of services that waits for the person
//! (`hvQDS`), its *Approve for every run while unchanged* confirmed in a modal only a person confirms. No runner runs
//! here: a person's answer reaches none and the card says so (`tests/run_services.rs` answers through a real one).

use lurq::app::events::with_synthetic_input;

use super::{click, opened_in, text};
use crate::support::Window;

/// The prefix of a service card's id in the chat.
const SERVICE: &str = "conversation-service-";

fn card(title: &str, prefix: &str) -> (Window, String) {
  let (window, id, _) = opened_in("service-cards", title, prefix);
  (window, id)
}

#[test]
fn service_crash_card_says_how_it_ended_what_was_stopped_and_its_last_lines() {
  let (mut window, card) = card("Invite people to a team server", SERVICE);
  assert_eq!(text(&mut window, &format!("{card}-title")), "cloud-api crashed");
  let meta = text(&mut window, &format!("{card}-meta"));
  assert!(
    meta.starts_with("exit code 1 · ") && meta.ends_with(" · 1.4 s after it started"),
    "{meta}"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-body")),
    "cloud-api exited on its own after it was ready. Orchester stopped cloud-web, which needs it; the agent got the \
     same facts."
  );
  assert_eq!(
    text(&mut window, &format!("{card}-lines")),
    "cloud-api connecting to postgres://127.0.0.1:41870/cloud cloud-api Error: db error: FATAL: the database system \
     is starting up orchester cloud-api exited with code 1"
  );
  for (part, words) in [
    ("restart", "Restart cloud-api"),
    ("open-log", "Open log"),
    ("edit", "Edit cloud-api"),
  ] {
    assert_eq!(text(&mut window, &format!("{card}-{part}")), words);
  }
  // A start the person asks for is their approval: an MCP client's press starts nothing.
  let restart = format!("{card}-restart");
  with_synthetic_input(|| click(&mut window, &restart));
  window.until("the person-only line", |window| {
    window.texts(&format!("{card}-person-only")).is_some()
  });
  assert_eq!(
    text(&mut window, &format!("{card}-person-only")),
    "Only a person starts cloud-api again here: a start you ask for is your approval of it."
  );
}

#[test]
fn service_start_without_docker_says_what_to_do_and_who_waits() {
  let (mut window, card) = card("Seed the team database", SERVICE);
  assert_eq!(
    text(&mut window, &format!("{card}-title")),
    "postgres can’t start: Docker isn’t running"
  );
  assert!(text(&mut window, &format!("{card}-meta")).starts_with("start_service postgres · "));
  assert_eq!(
    text(&mut window, &format!("{card}-body")),
    "postgres’s per-run data is a fresh database container, and Docker didn’t answer on this computer. Start Docker \
     Desktop, then Retry. cloud-api, team-server, team-web wait for it; the agent got the same message."
  );
  assert_eq!(
    text(&mut window, &format!("{card}-mac")),
    "On macOS any Docker that answers on the Docker socket works: Docker Desktop, OrbStack or colima."
  );
  assert_eq!(text(&mut window, &format!("{card}-restart")), "Retry");
  assert_eq!(text(&mut window, &format!("{card}-edit")), "Edit postgres");
  assert!(
    window.texts(&format!("{card}-open-log")).is_none(),
    "nothing ran, so no log"
  );
}

#[test]
fn service_readiness_timeout_says_what_was_asked_what_keeps_running_and_its_last_lines() {
  let (mut window, card) = card("Check the invite page", SERVICE);
  assert_eq!(
    text(&mut window, &format!("{card}-title")),
    "team-web wasn’t ready in 90 s"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-meta")),
    "readiness: HTTP 2xx or 3xx on /healthz"
  );
  assert_eq!(
    text(&mut window, &format!("{card}-body")),
    "Orchester asked http://127.0.0.1:41873/healthz until the timeout, got no 2xx or 3xx answer, and stopped \
     team-web. team-server, cloud-api, postgres keep running for this step."
  );
  let lines = text(&mut window, &format!("{card}-lines"));
  assert!(lines.starts_with("team-web > vite --port 41873"), "{lines}");
  assert!(
    lines.ends_with("orchester no answer on /healthz after 90 s · stopped team-web"),
    "{lines}"
  );
  for (part, words) in [
    ("restart", "Restart"),
    ("edit", "Edit readiness"),
    ("open-log", "Open log"),
  ] {
    assert_eq!(text(&mut window, &format!("{card}-{part}")), words);
  }
}

#[test]
fn service_slots_busy_says_what_the_start_waits_for_and_who_holds_the_slots() {
  let (mut window, card) = card("Run the team stack", SERVICE);
  assert_eq!(
    text(&mut window, &format!("{card}-title")),
    "Waiting for 4 Services at once slots"
  );
  let meta = text(&mut window, &format!("{card}-meta"));
  assert!(meta.contains(" · 03 Implement · since "), "{meta}");
  assert_eq!(
    text(&mut window, &format!("{card}-body")),
    "Stack team needs 4 and 2 are free. The run starts its services when 4 are free."
  );
  let bar = window
    .tree
    .get_element_by_id(&format!("{card}-bar"))
    .expect("the slot bar");
  assert_eq!(bar.children().len(), 8, "one slot each of the computer's");
  for (at, what) in [
    (0, "stack cloud · 3 slots · in 04 Test for 6 min"),
    (1, "desktop-qa, cloud-api, postgres · 3 slots · in 03 Implement"),
  ] {
    let holder = format!("{card}-holder-{at}");
    assert!(text(&mut window, &format!("{holder}-key")).starts_with("KON-"));
    assert_eq!(text(&mut window, &format!("{holder}-what")), what);
    assert_eq!(text(&mut window, &format!("{holder}-open")), "Open ↗");
  }
  assert_eq!(text(&mut window, &format!("{card}-change")), "Change Services at once");
}

/// What the card says when input from an MCP client gives a wider answer.
const WIDER_PERSON_ONLY: &str = "Only a person gives a wider answer: automation approves a request once or denies it.";

#[test]
fn service_start_approval_is_one_card_for_the_service_and_its_needs() {
  let (mut window, card) = card("Start the team web app", "cards-approval-");
  window.until("the request's rules", |window| {
    window
      .texts(&format!("{card}-program-team-server"))
      .is_some_and(|texts| texts.join(" ").contains("changed"))
  });
  assert_eq!(
    text(&mut window, &format!("{card}-title")),
    "Start service team-web and what it needs: team-server, cloud-api, postgres"
  );
  assert!(text(&mut window, &format!("{card}-meta")).starts_with("Builder · start_service · "));
  assert_eq!(
    text(&mut window, &format!("{card}-body")),
    "They start in this order and run these programs on this computer until 03 Implement ends. team-server changed \
     since you approved it. team-web never ran here before."
  );
  for (service, said) in [
    (
      "postgres",
      "1 postgres docker run --rm -p {{port}}:5432 postgres:16 approved, unchanged",
    ),
    (
      "cloud-api",
      "2 cloud-api cargo run -p orchester-cloud -- --port {{port}} approved, unchanged",
    ),
    (
      "team-server",
      "3 team-server cargo run -p orchester-team-server -- --port {{port}} changed: arguments",
    ),
    ("team-web", "4 team-web npm run dev --port {{port}} first start"),
  ] {
    assert_eq!(text(&mut window, &format!("{card}-program-{service}")), said);
  }
  assert_eq!(
    text(&mut window, &format!("{card}-slots-words")),
    "Takes 4 of 8 Services at once slots"
  );
  assert_eq!(text(&mut window, &format!("{card}-approve")), "Approve");
  assert_eq!(
    text(&mut window, &format!("{card}-every-run")),
    "Approve for every run while unchanged"
  );
  assert_eq!(text(&mut window, &format!("{card}-deny")), "Deny");
  assert_eq!(
    text(&mut window, &format!("{card}-deny-tells")),
    "Deny tells the agent no; it goes on without them"
  );
}

#[test]
fn service_start_approval_for_every_run_confirms_in_a_modal_only_a_person_confirms() {
  let (mut window, card) = card("Start the team web app", "cards-approval-");
  let (every_run, modal) = (format!("{card}-every-run"), format!("{card}-every-run-confirm"));
  click(&mut window, &every_run);
  window.until("the modal", |window| window.texts(&modal).is_some());
  assert_eq!(
    text(&mut window, &format!("{modal}-title")),
    "Approve these services for every run?"
  );
  assert_eq!(
    text(&mut window, &format!("{modal}-covers")),
    "Covers postgres, cloud-api, team-server, team-web: their program, arguments, variables and seed as they are now."
  );
  // Cancel takes it back.
  click(&mut window, &format!("{modal}-cancel"));
  window.until("the modal closed", |window| window.texts(&modal).is_none());
  // An MCP client's confirm makes no rule, even with both opt-ins on.
  let workspace = window.launch.workspace().expect("the session started");
  workspace
    .set_automation_answers_approvals(true)
    .expect("the approvals opt-in");
  workspace
    .set_automation_changes_permissions(true)
    .expect("the permissions opt-in");
  click(&mut window, &every_run);
  window.until("the modal again", |window| window.texts(&modal).is_some());
  with_synthetic_input(|| click(&mut window, &format!("{modal}-confirm")));
  let refused = format!("{card}-refused");
  window.until("why the client's answer was not taken", |window| {
    window.texts(&refused).is_some()
  });
  assert_eq!(text(&mut window, &refused), WIDER_PERSON_ONLY);
}
