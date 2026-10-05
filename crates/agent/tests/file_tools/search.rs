//! `search` with ripgrep and without it: match lines, context, globs, the index folder and the limits.

use std::fs;

use orchester_agent::FileTools;
use orchester_core::permission::RepositoryAccess;

use crate::{Layout, failed, ok, ripgrep, search_input, text};

fn with_ripgrep(layout: &Layout) -> FileTools {
  FileTools::new(layout.scope(RepositoryAccess::Write), Some(ripgrep()))
}

#[test]
fn files_search_with_ripgrep_prints_matches() {
  let layout = Layout::temporary();
  let tools = with_ripgrep(&layout);
  let outcome = tools.search(&search_input("hello", None, None, None));
  assert_eq!(outcome.activity, "3 matches in 3 files");
  let content = ok(outcome);
  assert!(content.contains("src/main.rs:2:    println!(\"hello\");"), "{content}");
  assert!(content.contains("README.md:2:hello there"), "{content}");
  assert!(!content.contains("target/") && !content.contains(".env"), "{content}");
  let context = ok(tools.search(&search_input("println", Some("src"), None, Some(1))));
  assert!(context.contains("src/main.rs-1-fn main() {"), "{context}");
  assert!(context.contains("src/main.rs:2:"), "{context}");
  let only_md = ok(tools.search(&search_input("hello", None, Some("*.md"), None)));
  assert_eq!(only_md, "README.md:2:hello there\n");
}

#[test]
fn files_search_reaches_the_code_index() {
  let layout = Layout::temporary();
  let index = text(&layout.index);
  let content = ok(with_ripgrep(&layout).search(&search_input("^hello", Some(&index), None, None)));
  assert!(content.contains("symbols.tsv:1:hello\tsrc/nested/lib.rs"), "{content}");
}

#[test]
fn files_search_reports_ripgrep_errors_and_limits() {
  let layout = Layout::temporary();
  let tools = with_ripgrep(&layout);
  let error = failed(tools.search(&search_input("(unclosed", None, None, None)));
  assert!(error.starts_with("ripgrep failed:"), "{error}");
  let many: String = (0..300).map(|n| format!("needle {n}\n")).collect();
  fs::write(layout.worktree.join("many.txt"), many).expect("file");
  let outcome = tools.search(&search_input("needle", None, None, None));
  assert_eq!(outcome.activity, "200 matches in 1 files");
  assert!(ok(outcome).contains("[truncated at 200 matches"));
}

#[test]
fn files_search_without_ripgrep_reads_the_pattern_literally() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let content = ok(tools.search(&search_input("println!(", None, None, Some(1))));
  assert!(content.contains("src/main.rs:2:    println!(\"hello\");"), "{content}");
  assert!(content.contains("src/main.rs-3-}"), "{content}");
  assert!(
    content.ends_with("searched as literal text, without .gitignore rules]"),
    "{content}"
  );
  let none = ok(tools.search(&search_input("hello", None, Some("*.py"), None)));
  assert!(none.starts_with("no matches"), "{none}");
}

#[test]
fn files_search_keeps_long_lines_short() {
  let layout = Layout::temporary();
  fs::write(
    layout.worktree.join("bundle.min.js"),
    format!("{}\n", "ab;".repeat(400_000)),
  )
  .expect("file");
  let content = ok(with_ripgrep(&layout).search(&search_input(".", Some("bundle.min.js"), None, None)));
  assert!(content.len() < 4 * 1024, "{} bytes", content.len());
  assert!(content.contains("[... omitted end of long line]"), "{content}");
  let literal = ok(
    layout
      .tools()
      .search(&search_input(";", Some("bundle.min.js"), None, None)),
  );
  assert!(literal.len() < 4 * 1024, "{} bytes", literal.len());
  assert!(literal.contains("[line cut at 600 characters]"), "{literal}");
}
