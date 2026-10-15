//! The review, commit and preservation compositions of Local integration (`YxF6B`, `kexy4`, `UgHeC`, `s4gj9`) in a
//! real dirty checkout: every listed snapshot and its labels are there, long paths and acknowledgements wrap
//! inside their column or card, and the 960 window keeps the narrow inspector beside the evidence.

use std::fs;

use lurq::node::element::ElementRef;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

const LONG: &str = "docs/integration/local-checkout-recovery-and-very-long-file-name-with-preservation-review-and-unique-final-suffix.md";
const NOTES: &str = "notes/retry-investigation.md";
const IMAGE: &str = "assets/retry-trace.png";

/// A checkout with a staged and a further unstaged edit of `local.txt`, an unstaged long path, and two untracked
/// files, one of them binary.
fn dirty() -> Page {
  let fixture = Fixture::named(false, "layout-");
  let root = fixture.root().to_owned();
  fs::create_dir_all(root.join("docs/integration")).expect("folder");
  fs::write(root.join(LONG), "first\n").expect("long path");
  git(&root, &["add", LONG]);
  git(&root, &["commit", "-m", "long path"]);
  fs::write(root.join(LONG), "first\nsecond\n").expect("unstaged long path");
  fs::write(root.join("local.txt"), "staged\n").expect("staged");
  git(&root, &["add", "local.txt"]);
  fs::write(root.join("local.txt"), "staged\nworking\n").expect("unstaged");
  fs::create_dir_all(root.join("notes")).expect("notes");
  fs::write(root.join(NOTES), "why the retry\n").expect("untracked");
  fs::create_dir_all(root.join("assets")).expect("assets");
  fs::write(root.join(IMAGE), [0_u8, 159, 146, 150, 0, 1, 2, 3]).expect("binary");
  Page::new(fixture)
}

struct Bounds {
  x: f32,
  y: f32,
  width: f32,
  height: f32,
}

impl Bounds {
  fn right(&self) -> f32 {
    self.x + self.width
  }
  fn bottom(&self) -> f32 {
    self.y + self.height
  }
  fn holds(&self, inner: &Self) -> bool {
    inner.x >= self.x - 0.5
      && inner.right() <= self.right() + 0.5
      && inner.y >= self.y - 0.5
      && inner.bottom() <= self.bottom() + 0.5
  }
}

fn bounds(page: &mut Page, id: &str) -> Bounds {
  let rect = page
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  Bounds {
    x: rect.x,
    y: rect.y,
    width: rect.width,
    height: rect.height,
  }
}

fn text(page: &mut Page, id: &str) -> String {
  page
    .window
    .texts(id)
    .unwrap_or_else(|| panic!("{id} is shown"))
    .join(" ")
}

/// Every element with an id under `id` lies inside its bounds.
fn nothing_outside(page: &mut Page, id: &str) {
  fn ids(element: ElementRef<'_>, into: &mut Vec<String>) {
    for child in element.children() {
      if let Some(id) = child.id() {
        into.push(id.to_owned());
      }
      ids(child, into);
    }
  }
  let mut inner = Vec::new();
  ids(page.window.tree.get_element_by_id(id).expect("shown"), &mut inner);
  assert!(!inner.is_empty(), "{id} holds identified elements");
  let outer = bounds(page, id);
  for child in inner {
    let Some(rect) = page
      .window
      .tree
      .get_element_by_id_mut(&child)
      .and_then(|element| element.bounds())
    else {
      continue;
    };
    let inner = Bounds {
      x: rect.x,
      y: rect.y,
      width: rect.width,
      height: rect.height,
    };
    assert!(
      outer.holds(&inner),
      "{child} in {id} reaches outside it: {} {} {} {}",
      inner.x,
      inner.y,
      inner.width,
      inner.height
    );
  }
}

fn resize(page: &mut Page, width: u32, height: u32) {
  page.window.tree.resize(width, height);
  page.window.pass();
}

fn review(page: &mut Page) {
  let long = format!("local-file-local_integration.unstaged-{LONG}");
  page.ready(&long);
  page.ready("local-file-diff-file-0");
}

#[test]
fn local_layout_review_lists_every_snapshot_and_wraps_long_paths_at_1440() {
  let mut page = dirty();
  resize(&mut page, 1440, 1080);
  review(&mut page);
  for row in [
    "local-row-local_integration.staged-local.txt".to_owned(),
    "local-row-local_integration.unstaged-local.txt".to_owned(),
    format!("local-row-local_integration.unstaged-{LONG}"),
    format!("local-row-local_integration.untracked-{NOTES}"),
    format!("local-row-local_integration.untracked-{IMAGE}"),
  ] {
    assert!(page.has(&row), "{row} is listed");
  }
  assert_eq!(text(&mut page, "local-files-title"), "Local changes");
  assert_eq!(
    text(&mut page, "local-phase-title"),
    "Delivery paused: main has local changes"
  );
  assert!(text(&mut page, "local-phase-trail").ends_with("Review local"));
  assert!(text(&mut page, "local-caption-local_integration.staged-local.txt").starts_with("HEAD → index · +1 −1"));
  assert!(
    text(&mut page, &format!("local-caption-local_integration.untracked-{IMAGE}")).starts_with("Binary · 8 bytes")
  );
  let long = bounds(&mut page, &format!("local-file-local_integration.unstaged-{LONG}"));
  let short = bounds(&mut page, "local-file-local_integration.staged-local.txt");
  assert!(
    long.height > short.height * 1.5,
    "the long path wraps onto another line"
  );
  nothing_outside(&mut page, "local-evidence");
  nothing_outside(&mut page, "local-side");
  assert_eq!(bounds(&mut page, "local-side").width, 336.0);
  assert_eq!(text(&mut page, "local-actions-title"), "Prepare this checkout");
  assert!(
    page.has("local-queue"),
    "no empty recovery record before an operation exists"
  );
  assert!(!page.has("local-recovery"));
  let card = bounds(&mut page, "local-actions");
  let commit = bounds(&mut page, "local-review-commit");
  assert!(
    (commit.width - (card.width - 32.0)).abs() < 1.0,
    "actions fill the card"
  );
}

#[test]
fn local_layout_960_keeps_the_narrow_inspector_beside_wrapped_evidence() {
  let mut page = dirty();
  resize(&mut page, 960, 1400);
  review(&mut page);
  let evidence = bounds(&mut page, "local-evidence");
  let side = bounds(&mut page, "local-side");
  assert_eq!(side.width, 248.0);
  assert!(
    (side.x - evidence.right() - 24.0).abs() < 1.0,
    "24 between evidence and inspector"
  );
  assert!(side.right() <= 960.0, "the inspector stays in the window");
  assert_eq!(text(&mut page, "local-actions-title"), "Prepare checkout");
  assert_eq!(text(&mut page, "local-phase-title"), "Delivery paused: main is dirty");
  assert_eq!(text(&mut page, "local-review-commit"), "Review selected commit");
  nothing_outside(&mut page, "local-evidence");
  nothing_outside(&mut page, "local-side");
  nothing_outside(&mut page, "local-identities");
}

fn select(page: &mut Page, ids: &[String]) {
  for id in ids {
    page.click(id);
  }
}

#[test]
fn local_layout_commit_review_lists_the_selected_snapshots_with_a_labeled_message() {
  let mut page = dirty();
  resize(&mut page, 1440, 1080);
  review(&mut page);
  select(
    &mut page,
    &[
      "local-select-local_integration.staged-local.txt".to_owned(),
      format!("local-select-local_integration.untracked-{NOTES}"),
    ],
  );
  page.click("local-review-commit");
  page.ready("local-commit-message-label");
  assert_eq!(text(&mut page, "local-files-title"), "Commit selection · 2 snapshots");
  assert!(page.has("local-row-local_integration.commitIndex-local.txt"));
  assert!(page.has(&format!("local-row-local_integration.commitUntracked-{NOTES}")));
  assert!(
    !page.has(&format!("local-row-local_integration.untracked-{IMAGE}")),
    "only the selection is listed"
  );
  assert_eq!(text(&mut page, "local-commit-message-label"), "Commit message");
  assert_eq!(bounds(&mut page, "local-commit-message-box").height, 38.0);
  assert!(
    page.has("local-selected-diff-file-0"),
    "the selected snapshots' diff follows the list"
  );
  assert!(text(&mut page, "local-remaining-body").contains("Unstaged local.txt"));
  assert!(text(&mut page, "local-phase-trail").ends_with("Commit review"));
  let scope = bounds(&mut page, "local-confirm-scope");
  assert!(scope.height > 20.0, "the acknowledgement wraps");
  assert_eq!(text(&mut page, "local-cancel-review"), "Back to selection");
  nothing_outside(&mut page, "local-actions");
  nothing_outside(&mut page, "local-evidence");
  assert!(
    page.has("local-recovery"),
    "the recorded operation shows its recovery record"
  );
}

#[test]
fn local_layout_preservation_review_names_the_selection_and_explains_no_discard() {
  let mut page = dirty();
  resize(&mut page, 1440, 1080);
  review(&mut page);
  select(
    &mut page,
    &[
      "local-select-local_integration.staged-local.txt".to_owned(),
      format!("local-select-local_integration.untracked-{IMAGE}"),
    ],
  );
  page.click("local-review-preserve");
  page.ready("local-preservation-name-label");
  assert_eq!(text(&mut page, "local-files-title"), "Preservation selection");
  assert!(page.has("local-row-local_integration.staged-local.txt"));
  assert!(page.has("local-row-local_integration.unstaged-local.txt"));
  assert!(page.has(&format!("local-row-local_integration.untracked-{IMAGE}")));
  assert!(!page.has(&format!("local-row-local_integration.untracked-{NOTES}")));
  assert_eq!(text(&mut page, "local-preservation-name-label"), "Preservation name");
  assert!(page.has("local-restore-after-merge"));
  assert!(page.has("local-no-discard"));
  assert!(
    !page.has("local-file-diff-file-0"),
    "the preservation review shows no diff"
  );
  assert_eq!(text(&mut page, "local-cancel-review"), "Cancel preparation");
  nothing_outside(&mut page, "local-actions");
  nothing_outside(&mut page, "local-evidence");
}

fn acknowledged(page: &mut Page) -> bool {
  let check = orchester_app::ui::icon::Icon::Check.glyph().to_string();
  super::integration_support::contains(
    page
      .window
      .tree
      .get_element_by_id("local-confirm-scope")
      .expect("acknowledgement"),
    &check,
  )
}

#[test]
fn local_layout_acknowledgement_is_the_persons_in_commit_and_preservation_reviews() {
  let mut page = dirty();
  resize(&mut page, 1440, 1080);
  // Window automation is on, so only the acknowledgement's own rule refuses it.
  page.window.link.input_gate().bind_window_control(true);
  review(&mut page);
  select(
    &mut page,
    &["local-select-local_integration.staged-local.txt".to_owned()],
  );
  page.click("local-review-preserve");
  page.ready("local-confirm-scope");
  lurq::app::events::with_synthetic_input(|| page.click("local-confirm-scope"));
  assert!(
    !acknowledged(&mut page),
    "window automation cannot acknowledge a preservation"
  );
  page.click("local-confirm-scope");
  assert!(acknowledged(&mut page), "the person's own input does");
  page.click("local-cancel-review");
  page.click("local-review-commit");
  page.ready("local-commit-message-label");
  lurq::app::events::with_synthetic_input(|| page.click("local-confirm-scope"));
  assert!(
    !acknowledged(&mut page),
    "window automation cannot acknowledge a commit"
  );
  page.click("local-confirm-scope");
  assert!(acknowledged(&mut page));
}
