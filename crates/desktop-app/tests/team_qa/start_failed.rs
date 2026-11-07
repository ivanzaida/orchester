//! A team start that fails for good (ORC-885): on the QA state `team-start-failed`, where the server builds KON-5's
//! start in a bundle schema one newer than this computer reads, Run claims the task, the start fails for good, the
//! claim is released, and the task page says why instead of "could not be carried out on this computer".
//!
//! `cargo test -p orchester-app --test team_qa team_qa_start_failed`

use orchester_app::ui::icon::Icon;

use super::{not_ready::on_task, team::State};

#[test]
fn team_qa_start_failed_says_the_bundle_schema_and_lets_the_claim_go() {
  let (mut window, server, _) = on_task(("team-start-failed", State::StartFailed), "KON-5");
  window.until("the runner", |window| {
    window
      .tree
      .get_element_by_id("task-run-icon")
      .and_then(|icon| icon.text_content().map(str::to_owned))
      .is_some_and(|glyph| glyph != Icon::Lock.glyph().to_string())
  });
  window.press("task-run");
  window.until("the notice", |window| window.texts("task-team-refused").is_some());

  assert!(
    window.text("task-team-refused").contains("Couldn't start KON-5's run"),
    "{}",
    window.text("task-team-refused")
  );
  let body = window.texts("task-team-refused").expect("the notice").join(" ");
  assert!(body.contains("Update Orchester on this computer"), "{body}");
  assert!(body.contains("your claim on the task was released"), "{body}");
  assert!(!body.contains("could not be carried out"), "{body}");

  let asked = |kind: &str| server.commands().iter().filter(|asked| *asked == kind).count();
  assert_eq!(asked("work.start"), 1, "the server made the work once");
  assert_eq!(asked("claim.release"), 1, "the claim was let go of");
}
