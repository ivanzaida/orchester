//! Orchester's `search_file` and `search_external_file`: literal text, file by file in path order, at most the files
//! asked for, with and without ripgrep; outside the worktree only the files the caller lets through.

use std::{fs, path::Path};

use orchester_agent::FileTools;
use orchester_core::{
  orchester_tools::{FileCall, SearchTerm},
  permission::RepositoryAccess,
};
use serde_json::json;

use crate::{Layout, line_edits::file_call, ok, ripgrep};

fn term(term: &str, limit_files: Option<u32>) -> SearchTerm {
  match file_call("search_file", &json!({"term": term, "limit_files": limit_files})) {
    FileCall::Search(search) => search,
    other => panic!("{other:?}"),
  }
}

fn both(layout: &Layout) -> [FileTools; 2] {
  [
    FileTools::new(layout.scope(RepositoryAccess::ReadOnly), Some(ripgrep())),
    FileTools::new(layout.scope(RepositoryAccess::ReadOnly), None),
  ]
}

/// Each file with its matching lines, in path order; a term in lower case matches any case; a pattern is text.
#[test]
fn files_search_term_lists_files_with_their_lines() {
  let layout = Layout::temporary();
  // Without ripgrep `.gitignore` is not read: the ignored build folder goes, so both searches see the same files.
  fs::remove_dir_all(layout.worktree.join("target")).expect("removed");
  fs::write(
    layout.worktree.join("src/notes.txt"),
    "Hello again\nnothing\nhello (x)\n",
  )
  .expect("file");
  for tools in both(&layout) {
    let found = ok(tools.search_term(&term("hello", None)));
    let shown: Vec<&str> = found.lines().filter(|line| !line.starts_with('[')).collect();
    assert_eq!(
      shown,
      [
        "README.md",
        "     2: hello there",
        "src/main.rs",
        "     2:     println!(\"hello\");",
        "src/nested/lib.rs",
        "     1: pub fn hello() {}",
        "src/notes.txt",
        "     1: Hello again",
        "     3: hello (x)"
      ],
      "{found}"
    );
    let cased = ok(tools.search_term(&term("Hello", None)));
    assert!(cased.starts_with("src/notes.txt\n     1: Hello again\n"), "{cased}");
    assert!(!cased.contains("README"), "{cased}");
    assert!(ok(tools.search_term(&term("(x)", None))).contains("hello (x)"));
    assert!(ok(tools.search_term(&term("nowhere at all", None))).starts_with("no matches\n"));
  }
}

/// At most `limit_files` files are listed, and the answer says more match; a file shows at most its first lines.
#[test]
fn files_search_term_stops_at_its_limits() {
  let layout = Layout::temporary();
  fs::remove_dir_all(layout.worktree.join("target")).expect("removed");
  fs::write(layout.worktree.join("many.txt"), "hello\n".repeat(100)).expect("file");
  for tools in both(&layout) {
    let found = ok(tools.search_term(&term("hello", Some(2))));
    assert!(
      found.contains("[more files match: only the first 2 are listed"),
      "{found}"
    );
    assert_eq!(
      found
        .lines()
        .filter(|line| !line.starts_with(' ') && !line.starts_with('['))
        .count(),
      2
    );
    let many = ok(tools.search_term(&term("hello", None)));
    let lines = many
      .lines()
      .filter(|line| line.starts_with("  ") && !line.contains('['))
      .count();
    assert_eq!(lines, 30 + 3, "{many}");
    assert!(many.contains("[more lines of this file match]"), "{many}");
  }
}

/// Outside the worktree the files are named by their path, and a file the caller does not let through is never read:
/// the answer says nothing of it, whatever it holds.
#[test]
fn files_search_outside_keeps_only_what_the_caller_lets_through() {
  let layout = Layout::temporary();
  fs::write(layout.outside.join("public.txt"), "secret word here\n").expect("file");
  let keep = |file: &Path| file.file_name().is_none_or(|name| name != "secret.txt");
  for tools in both(&layout) {
    let found = ok(tools.search_outside(&layout.outside, ("secret", 20), &keep));
    assert!(found.contains("public.txt\n     1: secret word here"), "{found}");
    assert!(!found.contains("secret.txt"), "{found}");
    assert!(!found.contains("files with matches"), "{found}");
    let refused = ok(tools.search_outside(&layout.outside, ("secret", 20), &|_| false));
    assert!(refused.starts_with("no matches\n"), "{refused}");
  }
}

/// `find_references` looks for a name as a whole word in exactly its case, with ripgrep and without.
#[test]
fn files_search_word_finds_a_name_as_a_whole_word() {
  let layout = Layout::temporary();
  fs::remove_dir_all(layout.worktree.join("target")).expect("removed");
  fs::write(
    layout.worktree.join("src/uses.rs"),
    "hello();\nhello_world();\nsay_hello();\nHello::new();\nlet x = hello;\n",
  )
  .expect("file");
  for tools in both(&layout) {
    let found = ok(tools.search_word("hello", 20));
    assert!(
      found.contains("src/uses.rs\n     1: hello();\n     5: let x = hello;\n"),
      "{found}"
    );
    assert!(
      !found.contains("hello_world") && !found.contains("say_hello") && !found.contains("Hello::new"),
      "{found}"
    );
    assert!(found.contains("     1: pub fn hello() {}"), "{found}");
  }
}
