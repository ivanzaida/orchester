//! What the composer offers in each state of the run on `task-states`: its placeholder, whom the message goes
//! to, the send button's words, the line over the box and the quiet line under it; *Attach* unavailable; and
//! a lock with the reason once no agent of the run reads messages.

use std::{
  process::Command,
  time::{Duration, Instant},
};

use orchester_app::ui::tooltip::TOOLTIP_DELAY;
use orchester_core::{store::WorktreeStore, time::Timestamp};
use orchester_store::ProjectDb;

use super::{click, detail, key_of, open, seeded, support, text};

/// The placeholder, the recipient pill, the send button's words, the context line and the quiet line.
struct Offer {
  placeholder: String,
  recipient: String,
  send: String,
  context: String,
  note: String,
}

fn offer(window: &mut support::Window) -> Offer {
  Offer {
    placeholder: text(window, "composer-input"),
    recipient: text(window, "composer-recipient-text"),
    send: text(window, "composer-send-label"),
    context: text(window, "composer-context-text"),
    note: text(window, "composer-note"),
  }
}

#[test]
fn composer_a_queued_task_sends_when_the_run_starts() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Localize the invoice emails");
  let offer = offer(&mut window);
  // No run yet: the first agent of the task's pipeline reads the message (`YIbm7`).
  assert_eq!(offer.placeholder, "Write to the first agent of the run…");
  let agent = offer
    .recipient
    .strip_suffix(" · not started")
    .unwrap_or_else(|| panic!("the first agent, not started: {}", offer.recipient));
  assert_eq!(offer.send, "Send when the run starts");
  assert_eq!(offer.note, format!("Queued · delivered to {agent} when the run starts"));
  assert!(
    offer.context.is_empty(),
    "nothing to answer or continue: {}",
    offer.context
  );
}

#[test]
fn composer_attach_is_unavailable_and_says_why() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Localize the invoice emails");
  let bounds = window
    .tree
    .get_element_by_id_mut("composer-attach")
    .and_then(|element| element.bounds())
    .expect("Attach is laid out");
  window
    .tree
    .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  window.pass();
  window
    .tree
    .tick_timers_at(Instant::now() + TOOLTIP_DELAY + Duration::from_millis(50));
  window.pass();
  assert_eq!(
    text(&mut window, "composer-attach-tooltip"),
    "Attaching files is not available yet."
  );
}

#[test]
fn composer_ready_for_review_sends_a_follow_up() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Show the signed-in account");
  let offer = offer(&mut window);
  assert_eq!(offer.placeholder, "Ask a question or request a follow-up…");
  assert_eq!(offer.send, "Send follow-up");
  assert!(
    offer.recipient.ends_with(" · resumes its session"),
    "{}",
    offer.recipient
  );
  assert!(
    offer.note.starts_with("Asking continues ") && offer.note.contains("session in the run’s worktree"),
    "{}",
    offer.note
  );
}

#[test]
fn composer_answers_the_question_that_waits_and_can_leave_the_answer() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Sync contacts with HubSpot");
  let offer = offer(&mut window);
  assert_eq!(offer.placeholder, "Write your answer…");
  assert_eq!(offer.send, "Send answer");
  assert!(offer.context.starts_with("Answering "), "{}", offer.context);
  assert!(offer.context.contains("’s question from "), "{}", offer.context);
  assert!(offer.recipient.ends_with(" · asked you"), "{}", offer.recipient);
  // × leaves the answer: the message is an ordinary one to the agent that asked.
  click(&mut window, "composer-leave");
  window.until("the answer left", |window| {
    window.tree.get_element_by_id("composer-context").is_none()
  });
  let left = super::states::offer(&mut window);
  assert_eq!(left.send, "Send");
  assert!(left.note.starts_with("Queued · "), "{}", left.note);
}

#[test]
fn composer_requests_changes_to_the_plan_that_waits() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Rate-limit messages from GitHub");
  let offer = offer(&mut window);
  assert_eq!(offer.placeholder, "Describe the changes the plan needs…");
  assert_eq!(offer.send, "Request changes");
  assert!(
    offer.context.starts_with("Changes to plan v1 · ") && offer.context.ends_with(" writes plan v2 from your note"),
    "{}",
    offer.context
  );
  assert!(
    offer.recipient.ends_with(" · waiting for approval"),
    "{}",
    offer.recipient
  );
  assert!(
    offer.note.is_empty(),
    "the plan's mode says all over the box: {}",
    offer.note
  );
}

#[test]
fn composer_a_failed_run_continues_with_instructions() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Export invoices as PDF");
  let offer = offer(&mut window);
  assert_eq!(offer.send, "Continue with instructions");
  assert!(
    offer.context.starts_with("Continue ") && offer.context.contains(" with your instructions · attempt "),
    "{}",
    offer.context
  );
  assert!(
    offer.note.starts_with("Your message continues this run"),
    "{}",
    offer.note
  );
}

#[test]
fn composer_a_paused_run_resumes_with_the_message_or_keeps_it() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Paginate the events API");
  let offer = offer(&mut window);
  assert_eq!(offer.send, "Resume and send");
  assert!(
    offer.note.starts_with("Resume and send resumes the run"),
    "{}",
    offer.note
  );
  assert!(
    window.tree.get_element_by_id("composer-send-options").is_some(),
    "a second way: Send without resuming"
  );
}

#[test]
fn composer_an_interrupted_run_continues_with_the_message() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Import contacts from CSV");
  assert_eq!(text(&mut window, "composer-send-label"), "Continue run and send");
}

#[test]
fn composer_a_run_stopped_at_its_limit_says_which_and_raises_it() {
  let (mut window, seeded) = seeded();
  open(
    &mut window,
    &seeded,
    "Explain an expired GitHub token in the status bar",
  );
  let offer = offer(&mut window);
  assert_eq!(offer.send, "Raise the limit and continue");
  assert!(
    offer.context.starts_with("Stopped at the token limit · used "),
    "{}",
    offer.context
  );
}

#[test]
fn composer_a_run_stopped_by_the_daily_budget_queues_the_message() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Retry failed exports nightly");
  let offer = offer(&mut window);
  assert_eq!(offer.send, "Send");
  assert!(offer.note.contains("today’s token budget"), "{}", offer.note);
}

#[test]
fn composer_a_canceled_run_runs_again_with_the_message() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Retry Slack notifications");
  let offer = offer(&mut window);
  assert_eq!(offer.send, "Run again with this message");
  assert_eq!(offer.placeholder, "Write to the first agent of the next run…");
  assert_eq!(offer.context, "No agent of this run reads messages any more");
}

#[test]
fn composer_a_merged_run_is_locked_with_the_reason() {
  let (mut window, seeded) = seeded();
  open(&mut window, &seeded, "Set up CI caching");
  assert_eq!(
    text(&mut window, "composer-closed-reason"),
    "This run's work was merged. Messages no longer reach its agents."
  );
  assert!(
    window.tree.get_element_by_id("composer-input").is_none(),
    "nothing to write in"
  );
}

#[test]
fn composer_a_run_whose_worktree_was_removed_is_locked_with_the_reason() {
  let (mut window, seeded) = seeded();
  let key = key_of(&seeded, "Show the signed-in account");
  let run = detail(&seeded, &key).run.expect("the task ran").id();
  // Removed as Remove worktree removes it: the folder goes, and the removal is recorded.
  let worktree = seeded.repo.join(".orchester").join("worktrees").join(run.to_string());
  let removed = Command::new("git")
    .current_dir(&seeded.repo)
    .args(["worktree", "remove", "--force"])
    .arg(&worktree)
    .status()
    .expect("git runs");
  assert!(removed.success(), "git worktree remove: {removed}");
  ProjectDb::open(&seeded.repo)
    .expect("the project database opens")
    .save_worktree_removal(run, Timestamp::now())
    .expect("the removal is recorded");
  open(&mut window, &seeded, "Show the signed-in account");
  window.until("the lock", |window| {
    window.tree.get_element_by_id("composer-closed").is_some()
  });
  assert!(
    text(&mut window, "composer-closed-reason").starts_with("The run’s worktree was removed"),
    "{}",
    text(&mut window, "composer-closed-reason")
  );
}
