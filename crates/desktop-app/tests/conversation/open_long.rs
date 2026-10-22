//! Opening a long run's Conversation (ORC-71): KON-12 of the `long-chats` scenario, its transcript 1,500 events
//! longer than the `conversation` scenario's (about 320 rows), opens at its end with only the rows in and near the
//! view built, and settles in a bounded number of frames. What the opening measured is written under
//! `.tmp/orc-71/profile/` (`support::profile`), with `--features perf_profile` with a lurq profile session's samples.

use orchester_app::routes::{ProjectPage, Route, TaskView};

use super::{
  ids, key_of, seeded,
  support::{Window, profile},
};

/// The most rows built after any frame: those in and near the view.
const MOST_ROWS: usize = 16;
/// The most frames that change something once the newest row shows, before nothing changes any more.
const MOST_LAYOUTS: usize = 4;

#[test]
fn timeline_a_long_run_opens_building_few_rows_and_settles() {
  let (mut window, seeded) = seeded("long-chats");
  let key = key_of(&seeded, "Add retry to webhook sender");
  window.navigation().go(Route::Project(
    seeded.project,
    ProjectPage::Task(key, TaskView::Conversation),
  ));
  #[cfg(feature = "perf_profile")]
  let session = profile::session::Session::start(&window);
  let built = |window: &mut Window| ids(window, "conversation-row:").len();
  // The scenario's last round, after the rest of the transcript.
  let opened = profile::open(&mut window, built, |window| {
    window
      .texts("conversation")
      .is_some_and(|texts| texts.iter().any(|text| text.contains("Round 299: the jitter bound")))
  });
  #[cfg(feature = "perf_profile")]
  let samples = session.end();
  #[cfg(not(feature = "perf_profile"))]
  let samples = String::new();
  profile::write("open-run", &opened, &samples);
  assert!(
    opened.most_built() <= MOST_ROWS,
    "at most {MOST_ROWS} rows are built: {opened:?}"
  );
  assert!(
    opened.layouts_to_settle() <= MOST_LAYOUTS,
    "it settles within {MOST_LAYOUTS} frames: {opened:?}"
  );
  assert!(
    window.tree.get_element_by_id("conversation-jump").is_none(),
    "it opens at its end, following"
  );
}
