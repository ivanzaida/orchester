//! Bones take the room their content will: a conversation's bones fit a short window over their composer, the
//! Changes panes fill the tab's height, and the Overview keeps its links and *Hold new starts* real among its bones.

use orchester_app::routes::{OverviewTab, ProjectPage, TaskTab, TaskView};

use crate::harness::Held;

/// A window shorter than the conversation's bones are tall.
const SHORT: (u32, u32) = (1440, 560);

#[test]
fn skeletons_a_conversations_bones_keep_its_composer_in_a_short_window() {
  let mut held = Held::new();
  held.window.tree.resize(SHORT.0, SHORT.1);
  let task = held.task.clone();
  held.page(ProjectPage::Task(task, TaskView::Conversation));
  held.skeleton("task-loading");
  let frame = held.bounds("task-frame");
  let bones = held.bounds("task-loading-bones");
  assert!(
    bones.y + bones.height <= frame.y + frame.height + 0.5,
    "the bones end inside the page: {} past {}",
    bones.y + bones.height,
    frame.y + frame.height
  );
}

#[test]
fn skeletons_the_changes_panes_fill_the_tabs_height() {
  let mut held = Held::new();
  let task = held.task.clone();
  held.page(ProjectPage::Task(task, TaskView::of(TaskTab::Changes)));
  held.skeleton("task-loading");
  let room = held.bounds("task-loading");
  let bones = held.bounds("task-loading-bones");
  assert!(
    (bones.height - room.height).abs() < 0.5,
    "the panes are as high as the room left: {} vs {}",
    bones.height,
    room.height
  );
}

#[test]
fn skeletons_the_overview_keeps_its_links_and_hold_among_the_bones() {
  let mut held = Held::new();
  held.page(ProjectPage::Overview(OverviewTab::default()));
  held.skeleton("overview-landed-bones");
  assert!(held.exists("overview-landed-done"), "Done ↗ stays real");
  assert!(held.exists("overview-delivery-open"), "Delivery ↗ stays real");
  assert!(held.exists("overview-queue-hold"), "Hold new starts stays real");
  assert!(
    held.exists("overview-merge-queue-reading-line"),
    "the merge queue's line stands"
  );
  assert!(held.exists("overview-at-work-bones") && held.exists("overview-queue-bones"));
}
