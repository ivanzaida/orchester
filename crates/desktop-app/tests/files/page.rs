//! What the page shows of a file and its versions, and what its controls do.
use std::fs;

use lurq::{app::events::with_synthetic_input, node::element::ElementRef};
use orchester_app::{
  routes::{FilesView, ProjectPage, Route},
  ui::icon::{Icon, glyph},
};
use orchester_core::id::RunId;
use orchester_workspace::files::{LineRange, Version};

use super::harness::Files;

/// The bounds of the element `id`.
fn bounds(files: &mut Files, id: &str) -> lurq::core::ElementRect {
  files
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn files_view(route: &Route) -> FilesView {
  match route {
    Route::Project(_, ProjectPage::Files(view)) => view.clone(),
    other => panic!("not the code viewer: {other:?}"),
  }
}

#[test]
fn files_shows_a_file_at_the_checkout_with_its_folders_open() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  assert_eq!(files.window.text("files-title"), "main.rs");
  files.window.until("the checkout's branch", |w| {
    w.text("files-meta-text").contains(" · main · ")
  });
  let meta = files.window.text("files-meta-text");
  assert!(meta.starts_with("Project checkout"), "{meta}");
  assert!(meta.ends_with("Rust · 120 lines · 3.4 KiB"), "{meta}");
  assert!(files.exists("files-tree:src") && files.exists("files-tree:src/main.rs"));
  assert!(files.exists("files-tree:src/ui"), "src is open, its folders listed");
  assert!(!files.exists("files-tree:src/ui/row.rs"), "src/ui stays closed");
  assert!(files.exists("files-line-1"), "the first lines are shown");
  assert!(!files.exists("files-line-120"), "the lines are virtualised");
  files.window.press("files-tree:src/ui");
  assert!(files.exists("files-tree:src/ui/row.rs"));
  files.window.press("files-tree:src/ui/row.rs");
  files.settle();
  files.window.until("row.rs", |w| w.text("files-title") == "row.rs");
}

#[test]
fn files_selects_the_lines_a_link_names_and_a_press_on_a_number() {
  let mut files = Files::new();
  files.go(FilesView::new(Some("src/main.rs"), Version::Checkout).with_line(LineRange::new(100, 102)));
  assert_eq!(files.window.text("files-selection"), "Lines 100–102 selected");
  files.window.until("the range in view", |w| {
    w.tree.get_element_by_id("files-line-101").is_some()
  });
  assert_eq!(files.window.text("files-copy"), "Copy lines");
  files.window.press("files-copy");
  let copied = files.stand_ins.copied.lock().clone();
  assert_eq!(copied.len(), 1);
  assert_eq!(copied[0].lines().count(), 3);
  assert!(copied[0].starts_with("  let value_100 = 100;"));
  files.window.press("files-line-number-101");
  files.window.until("the line selected", |w| {
    w.text("files-selection") == "Line 101 selected"
  });
  assert_eq!(files_view(&files.route()).line(), LineRange::new(101, 101));
}

#[test]
fn files_copy_and_open_in_editor_refuse_synthetic_input() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  with_synthetic_input(|| files.window.press("files-copy"));
  files.window.pass();
  assert!(files.stand_ins.copied.lock().is_empty(), "nothing was copied");
  assert!(files.window.text("files-refusal").contains("Only a person"));
  with_synthetic_input(|| files.window.press("files-open"));
  files.window.pass();
  assert!(files.window.text("files-refusal").contains("Only a person"));
}

#[test]
fn files_switches_between_the_checkout_and_the_base_branch() {
  let mut files = Files::new();
  fs::write(files.repo.join("src/ui/row.rs"), "pub struct Row;\npub struct Cell;\n").expect("an edit");
  files.open(Some("src/ui/row.rs"), Version::Checkout);
  files.window.until("the checkout's lines", |w| {
    w.text("files-meta-text").contains("2 lines")
  });
  files.window.press("files-version");
  files.window.until("the version menu", |w| {
    w.tree.get_element_by_id("files-version:base").is_some()
  });
  assert!(files.exists("files-version:checkout"));
  assert!(files.window.text("files-version:base").contains("main at"));
  files.window.press("files-version:base");
  files.settle();
  files
    .window
    .until("the base branch", |w| w.text("files-meta-text").contains("1 line"));
  assert_eq!(files_view(&files.route()).at(), &Version::Base);
  assert_eq!(files.window.text("files-version-label"), "Base branch");
  assert!(files.window.text("files-open").contains("Open in editor"));
  assert!(
    files
      .window
      .text("files-open-reason")
      .starts_with("A commit's file is not on disk"),
    "why it is unavailable is on screen"
  );
}

#[cfg(windows)]
#[test]
fn files_say_why_a_script_does_not_open_in_an_editor() {
  let mut files = Files::new();
  fs::write(files.repo.join("deploy.ps1"), "Write-Output 'deploy'\n").expect("a script");
  files.open(Some("deploy.ps1"), Version::Checkout);
  files
    .window
    .until("the script", |w| w.text("files-meta-text").contains("1 line"));
  assert_eq!(
    files.window.text("files-open-reason"),
    "Orchester does not open a file that your system would run instead of showing."
  );
  files.open(Some("src/main.rs"), Version::Checkout);
  assert!(!files.exists("files-open-reason"), "code opens");
}

#[test]
fn files_a_removed_worktree_says_so_and_offers_the_checkout() {
  let mut files = Files::new();
  files.open(Some("README.md"), Version::Run(RunId::generate()));
  assert!(files.exists("files-gone"));
  assert!(files.window.text("files-gone").contains("worktree no longer exists"));
  assert!(files.window.text("files-meta-text").contains("no longer exists"));
  files.window.press("files-gone-checkout");
  files.settle();
  assert_eq!(files_view(&files.route()).at(), &Version::Checkout);
  files.window.until("the checkout's file", |w| {
    w.tree.get_element_by_id("files-code").is_some()
  });
}

#[test]
fn files_refuses_paths_that_lead_out_and_says_why() {
  let mut files = Files::new();
  files.open(Some("../kontur/.env"), Version::Checkout);
  assert!(
    files
      .window
      .text("files-refused")
      .contains("This path is outside the project")
  );
  assert!(files.window.text("files-refused").contains("“..”"));
  assert_eq!(files.window.text("files-title"), "../kontur/.env");
  files.open(Some(".git/config"), Version::Checkout);
  assert!(files.window.text("files-refused").contains("This path is not shown"));
  files.open(Some("/etc/passwd"), Version::Checkout);
  assert!(files.window.text("files-refused").contains("absolute"));
}

#[test]
fn files_binary_and_missing_files_are_explained() {
  let mut files = Files::new();
  files.open(Some("logo.png"), Version::Checkout);
  assert!(files.window.text("files-binary").contains("logo.png is a binary file"));
  assert!(files.window.text("files-file-note").starts_with("Binary"));
  files.open(Some("src/gone.rs"), Version::Checkout);
  assert!(files.window.text("files-unreadable").contains("not in this version"));
  files.open(None, Version::Checkout);
  assert!(files.exists("files-no-file"));
  assert_eq!(files.window.text("files-title"), "Files");
}

#[test]
fn files_go_to_file_finds_a_file_by_its_name() {
  let mut files = Files::new();
  files.open(None, Version::Checkout);
  files.window.press("files-go-to");
  assert!(files.exists("files-go-to-search"));
  files.window.set("files-go-to-field", "row");
  files.window.until("the search's answer", |w| {
    w.tree.get_element_by_id("files-go-to:src/ui/row.rs").is_some()
  });
  assert_eq!(files.window.text("files-go-to-count"), "1 file matches");
  files.window.press("files-go-to:src/ui/row.rs");
  files.settle();
  files.window.until("row.rs", |w| w.text("files-title") == "row.rs");
  assert!(!files.exists("files-go-to-search"));
}

#[test]
fn files_find_counts_the_places_in_the_file() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-find-open");
  assert!(files.exists("files-find"));
  files.type_keys("LIMIT");
  files.window.until("the places found", |w| {
    w.text("files-find-field-box").contains("1 of 117")
  });
  files.window.press("files-find-next");
  assert!(files.window.text("files-find-field-box").contains("2 of 117"));
  files.window.press("files-find-case");
  files.window.until("no places in its case", |w| {
    w.text("files-find-field-box").contains("No results")
  });
  files.window.press("files-find-close");
  assert!(!files.exists("files-find"));
}

#[test]
fn files_cut_a_very_long_line_and_say_so() {
  let mut files = Files::new();
  let long = format!("short\n{}\n", "x".repeat(10_000));
  fs::write(files.repo.join("long.txt"), long).expect("a file with a long line");
  files.open(Some("long.txt"), Version::Checkout);
  files
    .window
    .until("its lines", |w| w.text("files-meta-text").contains("2 lines"));
  assert!(files.exists("files-line-cut"), "the long line says it was cut");
  assert_eq!(files.window.text("files-line-cut"), "Line shortened");
  let (marker, line) = (bounds(&mut files, "files-line-cut"), bounds(&mut files, "files-line-2"));
  assert!(
    marker.x + marker.width <= line.x + line.width + 0.5,
    "the line has room for the marker, so it scrolls into view: {marker:?} in {line:?}"
  );
}

/// Every text inside `element`, icon glyphs among them.
fn glyphs(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    glyphs(child, into);
  }
}

#[test]
fn files_tab_shows_the_kind_of_its_file() {
  let mut files = Files::new();
  for (file, kind) in [
    ("logo.png", Icon::FileImage),
    ("README.md", Icon::FileText),
    ("src/main.rs", Icon::FileCode),
  ] {
    files.open(Some(file), Version::Checkout);
    let mut shown = Vec::new();
    glyphs(
      files.window.tree.get_element_by_id("tab-strip").expect("the tabs"),
      &mut shown,
    );
    assert!(shown.contains(&glyph(kind)), "{file}'s tab shows {kind:?}");
  }
}

#[test]
fn files_find_says_it_is_searching_until_a_new_query_is_searched() {
  let mut files = Files::new();
  files.open(Some("src/main.rs"), Version::Checkout);
  files.window.press("files-find-open");
  files.type_keys("LIMIT");
  assert!(
    files.window.text("files-find-field-box").contains("Searching…"),
    "not \"No results\" before the search ran: {}",
    files.window.text("files-find-field-box")
  );
  files.window.until("the places found", |w| {
    w.text("files-find-field-box").contains("1 of 117")
  });
}

#[test]
fn files_find_marks_a_cut_line_whose_match_is_past_the_cut() {
  let mut files = Files::new();
  let long = format!("short\n{}needle\n", "x".repeat(5_000));
  fs::write(files.repo.join("long.txt"), long).expect("a file with a long line");
  files.open(Some("long.txt"), Version::Checkout);
  files
    .window
    .until("its lines", |w| w.text("files-meta-text").contains("2 lines"));
  assert_eq!(files.window.text("files-line-cut"), "Line shortened");
  files.window.press("files-find-open");
  files.type_keys("needle");
  files
    .window
    .until("the place found", |w| w.text("files-find-field-box").contains("1 of 1"));
  assert_eq!(files.window.text("files-line-cut"), "Match beyond the shortened part");
  let (marker, line) = (bounds(&mut files, "files-line-cut"), bounds(&mut files, "files-line-2"));
  assert!(
    marker.x + marker.width <= line.x + line.width + 0.5,
    "the longer words fit too: {marker:?} in {line:?}"
  );
}

#[cfg(windows)]
#[test]
fn files_say_why_a_path_with_a_comma_does_not_open_in_an_editor() {
  let mut files = Files::new();
  fs::create_dir_all(files.repo.join("evil.hta,z")).expect("a folder with a comma");
  fs::write(files.repo.join("evil.hta,z/notes.txt"), "notes\n").expect("notes");
  files.open(Some("evil.hta,z/notes.txt"), Version::Checkout);
  files
    .window
    .until("the notes", |w| w.text("files-meta-text").contains("1 line"));
  assert_eq!(
    files.window.text("files-open-reason"),
    "A file whose path has a comma does not open from here: Windows Explorer could read it as two."
  );
}
