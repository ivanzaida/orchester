//! The Pipelines page while its pipelines are read for the first time (`Pipelines · v2` P5 `TF9O3`): the title, *New
//! pipeline* and the tab names stay real around the bones of the counts, the list and the panel; the bones show only
//! after the delay, and the real page takes their place. The screen first waits for whether the project is shared, so
//! these tests let that read finish on another page and hold the reads again before the page opens.

use orchester_app::routes::{OverviewTab, ProjectPage};
use serde_json::json;

use crate::{harness::Held, mcp_client::Client};

/// The page's bones: the project's own read done, the pipelines' read held.
fn held_pipelines() -> Held {
  let mut held = Held::new();
  held.hold.set(false);
  held.page(ProjectPage::Overview(OverviewTab::default()));
  held.until("overview-header");
  held.window.pass();
  held.hold.set(true);
  held.page(ProjectPage::Pipelines);
  held
}

#[test]
fn skeletons_pipelines_show_the_lists_and_panels_bones_under_the_real_head_and_tabs() {
  let mut held = held_pipelines();
  held.skeleton("pipeline-hub-loading");
  for real in [
    "pipeline-hub-title",
    "pipeline-hub-new",
    "pipeline-hub-filter-all",
    "pipeline-hub-filter-archived",
  ] {
    assert!(held.exists(real), "{real} stays real");
  }
  for counted in ["all", "archived"] {
    let count = format!("pipeline-hub-filter-{counted}-count");
    assert!(held.exists(&count), "{counted}'s count keeps its place");
    // A bone holds only the digit's room, a clipped 0.
    assert_eq!(
      held.window.texts(&count).map(|words| words.concat()).as_deref(),
      Some("0"),
      "{counted}'s count is a bone, not a number"
    );
  }
  assert!(
    held.exists("pipeline-hub-loading-master-detail") && !held.exists("pipeline-hub-list"),
    "the list and the panel are bones in the master-detail's outline"
  );
  assert!(
    !held.exists("pipeline-hub-empty") && !held.exists("pipeline-hub-load-error"),
    "nothing pretends to be empty or failed while it is read"
  );
}

#[test]
fn skeletons_pipelines_real_hub_takes_the_bones_place_in_the_same_room() {
  let mut held = held_pipelines();
  held.skeleton("pipeline-hub-loading");
  let bones = held.bounds("pipeline-hub-loading-master-detail");
  held.release("pipeline-hub-master-detail");
  assert_ne!(
    held
      .window
      .texts("pipeline-hub-filter-all-count")
      .map(|words| words.concat())
      .as_deref(),
    Some("0"),
    "the count replaces its bone"
  );
  let read = held.bounds("pipeline-hub-master-detail");
  assert!(!held.exists("pipeline-hub-loading"), "the bones are gone");
  assert!(
    (read.y - bones.y).abs() < 0.5
      && (read.height - bones.height).abs() < 0.5
      && (read.width - bones.width).abs() < 0.5,
    "the body stands where its bones stood: {read:?} vs {bones:?}"
  );
}

#[test]
fn skeletons_pipelines_bones_are_named_for_automation_by_the_reading_words() {
  let mut held = held_pipelines();
  held.skeleton("pipeline-hub-loading");
  let client = Client::new(&mut held.window);
  let tree = client.call(&mut held.window, "lurq_read_tree", &json!({}));
  assert!(
    tree.contains("Reading the pipelines…"),
    "the skeleton's accessible name is the reading words: {tree}"
  );
}
