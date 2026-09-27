//! The unified diff reader: diffs GitHub served for `dtolnay/anyhow#463` (open, from a fork) and
//! `ivanzaida/lurq#5` and `#27` (merged; 35 files and new files in #27), and `git diff` of a synthetic repository
//! for what those pull requests do not contain: renames, a pure rename, a deletion, a binary
//! file, a mode change, quoted and spaced names, and a missing newline at the end of a file.

use orchester_core::diff::{
  Diff, DiffParseError, FileChange, FileDiff, LineKind, MAX_FILE_LINES, MAX_LINE_CHARS, ModeChange,
};

use crate::replay::fixture;

fn parse(text: &str) -> Diff {
  Diff::parse(text).expect("a readable diff")
}

fn file<'a>(diff: &'a Diff, path: &str) -> &'a FileDiff {
  diff
    .files()
    .iter()
    .find(|file| file.path() == path)
    .unwrap_or_else(|| panic!("{path} is in the diff"))
}

#[test]
fn diff_of_an_open_pull_request_has_its_files_hunks_and_line_numbers() {
  let diff = parse(fixture!("gh_api_compare_anyhow_463.stdout"));
  let paths: Vec<&str> = diff.files().iter().map(FileDiff::path).collect();
  assert_eq!(paths, ["src/error.rs", "tests/test_backtrace.rs"]);
  // As `gh pr view` counts them.
  assert_eq!((diff.additions(), diff.deletions()), (29, 0));
  let error = file(&diff, "src/error.rs");
  assert_eq!(error.change(), &FileChange::Modified);
  let hunk = &error.hunks()[0];
  assert_eq!(
    (hunk.old_start(), hunk.old_count(), hunk.new_start(), hunk.new_count()),
    (35, 6, 35, 21)
  );
  assert_eq!(hunk.section(), "impl Error {");
  let first = &hunk.lines()[0];
  assert_eq!(first.kind(), LineKind::Context);
  assert_eq!((first.old_number(), first.new_number()), (Some(35), Some(35)));
  assert_eq!(first.text(), "        Error::construct_from_std(error, backtrace)");
  let added = &hunk.lines()[3];
  assert_eq!(added.kind(), LineKind::Added);
  assert_eq!((added.old_number(), added.new_number()), (None, Some(38)));
  assert_eq!(
    added.text(),
    "    /// Create a new error object from any error type and a backtrace."
  );
  let last = hunk.lines().last().expect("lines");
  assert_eq!((last.old_number(), last.new_number()), (Some(40), Some(55)));
}

#[test]
fn diff_of_a_merged_pull_request_counts_what_github_counts() {
  let diff = parse(fixture!("gh_api_compare_lurq_27.stdout"));
  // `gh pr list` reports 35 files, +876 / −224 for this pull request.
  assert_eq!(diff.files().len(), 35);
  assert_eq!((diff.additions(), diff.deletions()), (876, 224));
  let added = diff
    .files()
    .iter()
    .filter(|file| file.change() == &FileChange::Added)
    .count();
  assert_eq!(added, 4);
  assert!(diff.files().iter().all(|file| file.omitted_lines() == 0));
}

#[test]
fn diff_of_a_small_merged_pull_request_matches_its_view() {
  let diff = parse(fixture!("gh_api_compare_lurq_5.stdout"));
  // `gh pr view 5` reports 2 files, +36 / −2.
  assert_eq!(diff.files().len(), 2);
  assert_eq!((diff.additions(), diff.deletions()), (36, 2));
}

#[test]
fn renames_deletions_binaries_and_modes_are_read_from_their_headers() {
  let diff = parse(fixture!("git_diff_synthetic.stdout"));
  let moved = file(&diff, "dir/moved.txt");
  assert_eq!(moved.change(), &FileChange::Renamed { similarity: Some(100) });
  assert_eq!(moved.old_path(), Some("moved.txt"));
  assert!(moved.hunks().is_empty());
  let renamed = file(&diff, "new_name.md");
  assert_eq!(renamed.change(), &FileChange::Renamed { similarity: Some(86) });
  assert_eq!(renamed.old_path(), Some("old_name.md"));
  assert_eq!((renamed.additions(), renamed.deletions()), (1, 1));
  let gone = file(&diff, "gone.txt");
  assert_eq!(gone.change(), &FileChange::Deleted);
  assert_eq!((gone.old_path(), gone.new_path()), (Some("gone.txt"), None));
  assert_eq!(gone.deletions(), 2);
  let image = file(&diff, "image.png");
  assert_eq!(image.change(), &FileChange::Added);
  assert!(image.is_binary());
  assert!(image.hunks().is_empty());
  let script = file(&diff, "run.sh");
  assert_eq!(script.change(), &FileChange::Modified);
  assert_eq!(
    script.mode(),
    Some(&ModeChange {
      old: "100644".to_owned(),
      new: "100755".to_owned()
    })
  );
  assert!(script.hunks().is_empty());
}

#[test]
fn quoted_and_spaced_names_are_read_as_written() {
  let diff = parse(fixture!("git_diff_synthetic.stdout"));
  assert_eq!(file(&diff, "naïve.txt").change(), &FileChange::Added);
  let spaced = file(&diff, "with space.txt");
  assert_eq!(spaced.new_path(), Some("with space.txt"));
  assert_eq!(spaced.hunks()[0].lines()[0].text(), "with space");
}

#[test]
fn missing_newline_at_end_of_file_marks_its_last_line() {
  let diff = parse(fixture!("git_diff_synthetic.stdout"));
  let notes = file(&diff, "notes.txt");
  let last = notes.hunks()[0].lines().last().expect("lines");
  assert_eq!(last.text(), "line seven");
  assert!(last.has_no_newline());
  let tail = file(&diff, "tail.txt");
  let lines = tail.hunks()[0].lines();
  assert_eq!(lines[0].kind(), LineKind::Removed);
  assert!(lines[0].has_no_newline());
  assert!(lines.iter().skip(1).all(|line| !line.has_no_newline()));
}

#[test]
fn carriage_returns_are_not_part_of_a_line() {
  let text = "diff --git a/dos.txt b/dos.txt\r\nindex a4b32e8..bdc5601 100644\r\n--- a/dos.txt\r\n+++ \
              b/dos.txt\r\n@@ -1,2 +1,2 @@\r\n crlf one\r\n-crlf two\r\n+crlf 2\r\n";
  let diff = parse(text);
  let lines = diff.files()[0].hunks()[0].lines();
  let texts: Vec<&str> = lines.iter().map(|line| line.text()).collect();
  assert_eq!(texts, ["crlf one", "crlf two", "crlf 2"]);
}

#[test]
fn a_very_large_file_keeps_its_first_lines_and_counts_the_rest() {
  let total = MAX_FILE_LINES + 250;
  let mut text = format!("diff --git a/big.txt b/big.txt\n--- a/big.txt\n+++ b/big.txt\n@@ -0,0 +1,{total} @@\n");
  for index in 0..total {
    text.push_str(&format!("+line {index}\n"));
  }
  let diff = parse(&text);
  let big = &diff.files()[0];
  assert_eq!(big.additions() as usize, total);
  assert_eq!(big.omitted_lines(), 250);
  assert_eq!(big.hunks()[0].lines().len(), MAX_FILE_LINES);
}

#[test]
fn a_very_long_line_is_cut_and_marked() {
  let long = "x".repeat(MAX_LINE_CHARS + 10);
  let text = format!("diff --git a/min.js b/min.js\n--- a/min.js\n+++ b/min.js\n@@ -1 +1 @@\n-a\n+{long}\n");
  let diff = parse(&text);
  let lines = diff.files()[0].hunks()[0].lines();
  assert!(!lines[0].is_cut());
  assert!(lines[1].is_cut());
  assert_eq!(lines[1].text().chars().count(), MAX_LINE_CHARS);
}

#[test]
fn an_empty_diff_has_no_files() {
  assert!(parse("").files().is_empty());
}

#[test]
fn text_that_is_not_a_diff_is_refused_with_its_line() {
  assert_eq!(
    Diff::parse("<!DOCTYPE html>\n"),
    Err(DiffParseError::UnexpectedLine { line: 1 })
  );
  assert_eq!(
    Diff::parse("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ nonsense @@\n"),
    Err(DiffParseError::HunkHeader { line: 4 })
  );
  assert_eq!(
    Diff::parse("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,3 +1,3 @@\n a\n"),
    Err(DiffParseError::ShortHunk { line: 4 })
  );
}
