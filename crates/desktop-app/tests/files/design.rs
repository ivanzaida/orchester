//! The code viewer at the design's sizes and in its states (`o1Uea4`, `kadDh`, `y29f1V`, `b7RGL`, `mQeh3`), at
//! the window's 1440: what design verification measured, held by the layout.
use std::fs;

use orchester_app::routes::FilesView;
use orchester_workspace::files::{LineRange, Version};

use super::harness::{Files, git};

/// `a` is `b` within half a pixel.
fn near(a: f32, b: f32, what: &str) {
  assert!((a - b).abs() <= 0.5, "{what}: {a} is not {b}");
}

#[test]
fn files_say_why_open_in_editor_is_unavailable_on_a_line_of_its_own_under_the_buttons() {
  let mut files = Files::new();
  files.open(Some("logo.png"), Version::Checkout);
  files
    .window
    .until("the binary file", |w| w.text("files-meta-text").contains("Binary"));
  let (reason, actions, header) = (
    files.bounds("files-open-reason"),
    files.bounds("files-actions"),
    files.bounds("files-header"),
  );
  assert_eq!(
    files.window.text("files-open-reason"),
    "Only text and code files open in an editor from here."
  );
  near(
    reason.y,
    actions.y + actions.height + 6.0,
    "6 under the buttons (`mQeh3`)",
  );
  near(
    reason.x + reason.width,
    actions.x + actions.width,
    "at the buttons' end",
  );
  assert!(
    reason.x >= header.x && reason.x + reason.width <= header.x + header.width - 32.0 + 0.5,
    "the whole reason is inside the header, whatever the meta line holds: {reason:?} in {header:?}"
  );
}

#[test]
fn files_header_keeps_its_height_while_open_in_editor_says_why_it_is_unavailable() {
  let mut files = Files::new();
  files.open(Some("README.md"), Version::Checkout);
  assert!(!files.exists("files-open-reason"));
  let without = files.bounds("files-header").height;
  files.open(Some("logo.png"), Version::Checkout);
  files.window.until("the reason", |w| {
    w.tree.get_element_by_id("files-open-reason").is_some()
  });
  let with = files.bounds("files-header").height;
  near(without, 105.0, "the header without a reason (`TpYaM`)");
  near(with, without, "the reason beside the meta line, as in `mQeh3`");
  let (reason, meta) = (files.bounds("files-open-reason"), files.bounds("files-meta"));
  assert!(
    reason.x >= meta.x + meta.width - 0.5,
    "the meta line ends before the reason: {meta:?}, {reason:?}"
  );
}

#[test]
fn files_header_tree_and_find_bar_have_the_designs_sizes() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  near(files.bounds("files-header").height, 105.0, "the header (`TpYaM`)");
  near(files.bounds("files-crumbs").height, 16.0, "the crumbs (`gxF9b`)");
  near(files.bounds("files-meta").height, 15.0, "the meta line (`F0BHDL`)");
  let head = files.bounds("files-tree-head");
  near(head.height, 38.0, "the tree's head (`gENuK`)");
  near(
    files.bounds("files-tree:src").y,
    head.y + head.height,
    "the first row under the head",
  );
  files.window.press("files-find-open");
  let (code, field, case, previous) = (
    files.bounds("files-code"),
    files.bounds("files-find-field-box"),
    files.bounds("files-find-case"),
    files.bounds("files-find-previous"),
  );
  near(field.x - code.x, 520.0, "the find field (`kWlG3`)");
  near(case.width, 26.0, "*Aa* (`O7iJW`)");
  near(
    previous.x - (case.x + case.width),
    6.0,
    "6 between the find bar's buttons",
  );
}

#[test]
fn files_version_menu_rows_are_47_apart() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-version");
  files.window.until("the version menu", |w| {
    w.tree.get_element_by_id("files-version:base").is_some()
  });
  let (checkout, base) = (
    files.bounds("files-version:checkout"),
    files.bounds("files-version:base"),
  );
  near(base.y - checkout.y, 47.0, "the rows' pitch (`kadDh`)");
}

#[test]
fn files_go_to_file_lists_the_versions_files_before_anything_is_typed_in_44_high_rows() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-go-to");
  files.window.until("the files", |w| {
    w.tree.get_element_by_id("files-go-to:README.md").is_some()
  });
  let count = files.window.text("files-go-to-count");
  assert!(
    count.ends_with(" files") && !count.contains("typed"),
    "with nothing typed, how many files there are: {count}"
  );
  near(files.bounds("files-go-to:README.md").height, 44.0, "a result (`MfxMF`)");
}

#[test]
fn files_a_links_lines_have_about_13_lines_above_them_in_view() {
  let mut files = Files::new();
  let text: String = (1..=600)
    .map(|line| {
      format!(
        "line {line}
"
      )
    })
    .collect();
  fs::write(files.repo.join("long.rs"), text).expect("a long file");
  files.go(FilesView::new(Some("long.rs"), Version::Checkout).with_line(LineRange::new(441, 442)));
  files.window.until("the range in view", |w| {
    w.tree.get_element_by_id("files-line-441").is_some()
  });
  let (code, line) = (files.bounds("files-code"), files.bounds("files-line-441"));
  // Under the file's 40 high head, about 13 lines of 20 above the range, as `o1Uea4` shows 428 for 441.
  let above = (line.y - code.y - 40.0) / 20.0;
  assert!((13.0..=14.0).contains(&above), "{above} lines above the range");
}

#[test]
fn files_tree_scrolls_to_the_file_shown_and_keeps_its_scrollbar_beside_the_rows() {
  let mut files = Files::new();
  for at in 0..60 {
    fs::write(files.repo.join(format!("f{at:02}.md")), "f\n").expect("a file");
  }
  fs::write(files.repo.join("zz.md"), "last\n").expect("the last file");
  git(&files.repo, &["add", "."]);
  git(&files.repo, &["commit", "--quiet", "-m", "many"]);
  files.open(Some("zz.md"), Version::Checkout);
  files.window.until("the file's row scrolled into view", |w| {
    w.tree.get_element_by_id("files-tree:zz.md").is_some()
  });
  let (row, gutter, panel) = (
    files.bounds("files-tree:zz.md"),
    files.bounds("files-tree-gutter:zz.md"),
    files.bounds("files-tree-panel"),
  );
  near(
    gutter.x + gutter.width - (row.x + row.width),
    8.0,
    "an 8 gutter after the row",
  );
  assert!(
    gutter.x + gutter.width >= panel.x + panel.width - 1.5,
    "the list, and its scrollbar, reach the panel's edge: {gutter:?} in {panel:?}"
  );
}

#[test]
fn files_a_move_to_another_file_closes_the_version_menu_and_go_to_file() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-version");
  files.window.until("the version menu", |w| {
    w.tree.get_element_by_id("files-version-menu").is_some()
  });
  files.open(Some("README.md"), Version::Checkout);
  assert!(!files.exists("files-version-menu"), "the menu closed");
  files.window.press("files-go-to");
  files.window.set("files-go-to-field", "row");
  assert!(files.exists("files-go-to-search"));
  files.open(Some("src/main.rs"), Version::Base);
  assert!(!files.exists("files-go-to-search"), "Go to file closed");
  files.window.press("files-go-to");
  assert!(
    files.window.text("files-go-to-count").ends_with(" files"),
    "it opens again without the old query"
  );
}

#[test]
fn files_meta_line_shortens_the_checkouts_folder_and_keeps_the_files_facts() {
  let mut files = Files::new();
  files.window.tree.resize(2400, 900);
  files.open(Some("src/main.rs"), Version::Checkout);
  files
    .window
    .until("the facts", |w| w.text("files-meta-text").contains("120 lines"));
  let whole = files.bounds("files-meta-facts").width;
  files.window.tree.resize(1440, 900);
  files.window.pass();
  let (facts, meta) = (files.bounds("files-meta-facts"), files.bounds("files-meta"));
  near(
    facts.width,
    whole,
    "the branch, language, lines and size uncut at 1440 (`o1Uea4`)",
  );
  assert!(
    facts.x + facts.width <= meta.x + meta.width + 0.5,
    "{facts:?} in {meta:?}"
  );
  let folder = files.window.text("files-meta-folder");
  assert!(
    folder.contains('…'),
    "the long folder shortened in its middle: {folder}"
  );
  assert!(files.window.text("files-meta-facts").contains("120 lines"));
}

#[test]
fn files_actions_stay_inside_the_header_at_960() {
  let mut files = Files::new();
  files.window.tree.resize(960, 640);
  files.open(Some("logo.png"), Version::Checkout);
  files.window.until("the reason", |w| {
    w.tree.get_element_by_id("files-open-reason").is_some()
  });
  let (header, actions, open) = (
    files.bounds("files-header"),
    files.bounds("files-actions-column"),
    files.bounds("files-open"),
  );
  let inside = header.x + header.width - 32.0 + 0.5;
  assert!(
    actions.x + actions.width <= inside && open.x + open.width <= inside,
    "the actions in the header's padding: {actions:?}, {open:?} in {header:?}"
  );
  let reason = files.bounds("files-open-reason");
  assert!(reason.x + reason.width <= inside, "{reason:?}");
}
