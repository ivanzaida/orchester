//! A pull request (08.02 `Rrmhf`), opened from the inbox and driven with the fake `gh` of the
//! inbox tests: `gh pr view` shaped like the captured one, and the synthetic `git diff` fixture as
//! the compared diff (a Rust file, renames, a deletion, a binary file, a mode change). The real
//! `gh` never runs, and Open in GitHub is not pressed.

use std::sync::Arc;

use lurq::node::element::ElementRef;

use crate::{
  github::{SIGNED_IN, contains_text, shows},
  harness::Desktop,
  pull_requests::{FakeGh, NETWORK, OPEN_PULL_REQUESTS, failed, linked, ok, open_pull_requests, settle_on},
};

const VIEW: &str = "pr view 42 -R github.com/ivanzaida/kontur --json \
                    number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,additions,deletions,\
                    changedFiles,updatedAt,reviewDecision,statusCheckRollup,baseRefOid";
const BASE: &str = "1111111111111111111111111111111111111111";
const COMPARE: &str = "api --allow-escape-sequences --hostname github.com -H Accept: application/vnd.github.diff \
                       repos/ivanzaida/kontur/compare/1111111111111111111111111111111111111111...\
                       8f3a2c1d4e5f60718293a4b5c6d7e8f901234567";
const DIFF: &str = include_str!("../../../core/tests/fixtures/github/git_diff_synthetic.stdout");
const NOT_FOUND: &str = include_str!("../../../core/tests/fixtures/github/gh_pr_view_not_found.stderr");
const COMPARE_NOT_FOUND: &str = include_str!("../../../core/tests/fixtures/github/gh_api_compare_not_found.stderr");
const CONTEXT: &str = "pull-request-context";

/// `gh pr view 42` of the synthetic #42 of the list: the list entry with its base revision.
fn view(changed_files: u32) -> String {
  let list: serde_json::Value = serde_json::from_str(OPEN_PULL_REQUESTS).expect("the synthetic list");
  let mut entry = list[0].clone();
  entry["baseRefOid"] = BASE.into();
  entry["changedFiles"] = changed_files.into();
  entry.to_string()
}

fn signed_in_with_detail() -> Arc<FakeGh> {
  let gh = FakeGh::signed_in(SIGNED_IN, Ok(ok(OPEN_PULL_REQUESTS)));
  gh.answer(VIEW, Ok(ok(&view(6))));
  gh.answer(COMPARE, Ok(ok(DIFF)));
  gh
}

fn compares(gh: &FakeGh) -> usize {
  gh.ran.lock().iter().filter(|args| args.as_str() == COMPARE).count()
}

/// The inbox, synced, then pull request #42.
fn open_detail(gh: &Arc<FakeGh>) -> Desktop {
  let (mut desktop, _) = linked(gh, "ivanzaida");
  open_pull_requests(&mut desktop);
  settle_on(&mut desktop, "pull-requests-list", "Add GitHub authentication");
  desktop.click("pull-request-row");
  desktop.wait_for("pull-request");
  desktop
}

/// The first text element under `root` whose text is `text`.
fn text_element<'a>(root: ElementRef<'a>, text: &str) -> Option<ElementRef<'a>> {
  if root.text_content() == Some(text) {
    return Some(root);
  }
  root.children().into_iter().find_map(|child| text_element(child, text))
}

#[test]
fn pull_request_opens_from_the_inbox_with_its_files_and_context() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, CONTEXT, "Synced just now");
  assert!(shows(
    &mut desktop,
    CONTEXT,
    "PR #42 · orc/github-auth → main · Head 8f3a2c1 · Synced just now"
  ));
  assert!(shows(&mut desktop, "pull-request-heading", "Add GitHub authentication"));
  assert!(shows(&mut desktop, "pull-request-tab-files", "11"));
  for path in ["dir/moved.txt", "gone.txt", "image.png", "main.rs", "with space.txt"] {
    let id = format!("pull-request-file:{path}");
    desktop.wait_for(&id);
  }
  assert!(shows(&mut desktop, "pull-request-files", "M · +2 / −1"));
  // The first file is selected: a pure rename says its content did not change.
  assert!(shows(&mut desktop, "pull-request-diff", "dir/moved.txt"));
  assert!(shows(&mut desktop, "pull-request-diff", "Renamed from moved.txt"));
  assert!(shows(&mut desktop, "pull-request-diff", "The content did not change"));
  assert_eq!(compares(&gh), 1);
}

#[test]
fn pull_request_file_shows_its_hunks_and_highlighted_lines() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  desktop.wait_for("pull-request-file:main.rs");
  desktop.click("pull-request-file:main.rs");
  settle_on(&mut desktop, "pull-request-diff", "@@ −1,3 +1,4 @@");
  desktop.settle("main.rs highlighted", |tree| {
    tree
      .get_element_by_id("pull-request-diff")
      .is_some_and(|diff| contains_text(diff, "world") && text_element(diff, "fn").is_some())
  });
  // Tokens are drawn in the `ExtraColor::Code*` role their class names (`tests/theme.rs` checks
  // the role values against Pencil).
  desktop.settle("the highlighted rows", |tree| {
    let diff = tree.get_element_by_id("pull-request-diff").expect("the diff");
    text_element(diff, "fn").is_some_and(|keyword| keyword.has_class("code-keyword"))
  });
  let diff = desktop.tree.get_element_by_id("pull-request-diff").expect("the diff");
  assert!(text_element(diff, "\"world\"").is_some_and(|token| token.has_class("code-string")));
  assert!(text_element(diff, "let").is_some_and(|token| token.has_class("code-keyword")));
  assert!(text_element(diff, "main").is_some_and(|token| token.has_class("code-function")));
}

#[test]
fn pull_request_notes_binary_and_mode_changes() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  desktop.wait_for("pull-request-file:image.png");
  desktop.click("pull-request-file:image.png");
  settle_on(
    &mut desktop,
    "pull-request-diff",
    "Binary file: its changes are not shown",
  );
  desktop.click("pull-request-file:run.sh");
  settle_on(
    &mut desktop,
    "pull-request-diff",
    "File mode changed from 100644 to 100755",
  );
  desktop.press("pull-request-file-search");
  assert_eq!(desktop.focused().as_deref(), Some("pull-request-file-search"));
  desktop.type_text("gone");
  assert!(shows(&mut desktop, "pull-request-files", "gone.txt"));
  assert!(desktop.tree.get_element_by_id("pull-request-file:main.rs").is_none());
}

#[test]
fn pull_request_refresh_reuses_the_diff_of_an_unchanged_head() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, CONTEXT, "Synced just now");
  assert_eq!(compares(&gh), 1);
  desktop.click("pull-request-refresh");
  desktop.settle("the second view", |_| {
    gh.ran.lock().iter().filter(|args| args.as_str() == VIEW).count() == 2
  });
  settle_on(&mut desktop, CONTEXT, "Synced just now");
  assert_eq!(compares(&gh), 1, "the stored diff of the same head is reused");
}

#[test]
fn pull_request_keeps_its_diff_when_github_is_unreachable() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, CONTEXT, "Synced just now");
  gh.answer(VIEW, Ok(failed(NETWORK)));
  desktop.click("pull-request-refresh");
  settle_on(&mut desktop, CONTEXT, "Last synced just now · GitHub unreachable");
  assert!(shows(&mut desktop, "pull-request-notice", "Couldn’t reach GitHub"));
  assert!(shows(&mut desktop, "pull-request-diff", "dir/moved.txt"));
}

#[test]
fn pull_request_that_does_not_exist_says_so() {
  let gh = signed_in_with_detail();
  gh.answer(VIEW, Ok(failed(NOT_FOUND)));
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, "pull-request", "Pull request #42 was not found");
  assert!(shows(
    &mut desktop,
    "pull-request",
    "GitHub has no pull request #42 in ivanzaida / kontur that ivanzaida can see."
  ));
  assert_eq!(compares(&gh), 0);
}

#[test]
fn pull_request_too_large_is_not_fetched() {
  let gh = signed_in_with_detail();
  gh.answer(VIEW, Ok(ok(&view(301))));
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, "pull-request", "Too large to show here");
  assert!(shows(
    &mut desktop,
    "pull-request",
    "This pull request changes 301 files"
  ));
  // Numbers as the locale groups them, and the tab still counts the files GitHub reported.
  assert!(shows(
    &mut desktop,
    "pull-request",
    "up to 300 files and 20,000 changed lines"
  ));
  assert!(shows(&mut desktop, "pull-request-tab-files", "301"));
  assert_eq!(compares(&gh), 0);
}

#[test]
fn pull_request_read_without_its_diff_is_synced_and_says_why() {
  let gh = signed_in_with_detail();
  gh.answer(COMPARE, Ok(failed(COMPARE_NOT_FOUND)));
  let mut desktop = open_detail(&gh);
  settle_on(&mut desktop, "pull-request", "The diff could not be read");
  assert!(shows(
    &mut desktop,
    CONTEXT,
    "PR #42 · orc/github-auth → main · Head 8f3a2c1 · Synced just now"
  ));
  // GitHub's own count of the changed files stands in for the unread diff's.
  assert!(shows(&mut desktop, "pull-request-tab-files", "6"));
}

#[test]
fn pull_request_breadcrumb_leads_back_to_the_inbox() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  assert!(shows(
    &mut desktop,
    "pull-request-breadcrumb",
    "Kontur  /  Pull requests  /  #42"
  ));
  desktop.click("pull-request-breadcrumb");
  desktop.wait_for("pull-requests-list");
  // Only Find a file takes focus on the pull request screen.
  let mut desktop = open_detail(&gh);
  desktop.wait_for("pull-request-file-search");
  desktop.press("pull-request-file-search");
  assert_eq!(desktop.focused().as_deref(), Some("pull-request-file-search"));
  desktop.tab();
  assert_eq!(desktop.focused().as_deref(), Some("pull-request-file-search"));
}

fn width(desktop: &mut Desktop, id: &str) -> f32 {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
    .width
}

#[test]
fn pull_request_rows_reach_across_the_diff() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  desktop.wait_for("pull-request-file:main.rs");
  desktop.click("pull-request-file:main.rs");
  settle_on(&mut desktop, "pull-request-diff", "@@ −1,3 +1,4 @@");
  // The file list is 192 wide (`h8PZJa`); short lines still color the diff's full width.
  assert!((width(&mut desktop, "pull-request-files") - 192.0).abs() < 0.5);
  let diff = width(&mut desktop, "pull-request-diff");
  for row in ["pull-request-hunk", "pull-request-line"] {
    assert!((width(&mut desktop, row) - diff).abs() < 0.5, "{row}");
  }
}

#[test]
fn pull_request_back_returns_to_the_inbox_and_forward_reopens_it() {
  let gh = signed_in_with_detail();
  let mut desktop = open_detail(&gh);
  let back = lurq::app::events::MouseButton::Other(4);
  desktop.tree.mouse_up(720.0, 480.0, back);
  desktop.settle("the inbox", |tree| {
    tree.get_element_by_id("pull-request").is_none() && tree.get_element_by_id("pull-requests-list").is_some()
  });
  desktop
    .tree
    .mouse_up(720.0, 480.0, lurq::app::events::MouseButton::Other(5));
  desktop.wait_for("pull-request");
}
