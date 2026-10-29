//! The run's first line while Autopilot answers its requests (`docs/design/approval-modes.md`). Agent profiles have no
//! approval mode yet, so every run is in Manual by its profile and the line stays away, also in a project that
//! allows Autopilot. `cargo test -p orchester-app --test conversation autopilot`.

use orchester_core::run::AllowAutopilot;
use orchester_store::ProjectDb;

use super::{all_text, key_of, open, seeded, to_start};

const LINE: &str = "conversation-autopilot";

#[test]
fn autopilot_line_is_gone_while_the_run_is_in_manual() {
  let (mut window, seeded) = seeded("conversation");
  ProjectDb::open(&seeded.repo)
    .expect("open")
    .save_allow_autopilot(AllowAutopilot::ON)
    .expect("allow autopilot");
  open(
    &mut window,
    &seeded,
    &key_of(&seeded, "Add retry to webhook sender"),
    "conversation-chat",
  );
  to_start(&mut window);
  window.until("the timeline's start", |window| {
    all_text(window).contains("01–04 · 4 steps done")
  });
  assert!(
    window.tree.get_element_by_id(LINE).is_none(),
    "the run is in Manual by its profile"
  );
}
