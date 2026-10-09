//! `list_files` with ripgrep (`.gitignore` respected) and without it (a walk that says so).

use orchester_agent::FileTools;
use orchester_core::permission::RepositoryAccess;

use crate::{Layout, failed, list_input, ok, ripgrep, text};

#[test]
fn files_list_with_ripgrep_respects_gitignore() {
  let layout = Layout::temporary();
  let tools = FileTools::new(layout.scope(RepositoryAccess::Write), Some(ripgrep()));
  let outcome = tools.list_files(&list_input(None, None));
  assert_eq!(outcome.activity, "3 files");
  assert_eq!(ok(outcome), "README.md\nsrc/main.rs\nsrc/nested/lib.rs");
  let rust = ok(tools.list_files(&list_input(Some("*.rs"), Some("src"))));
  assert_eq!(rust, "src/main.rs\nsrc/nested/lib.rs");
}

#[test]
fn files_list_without_ripgrep_walks_and_says_so() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let all = ok(tools.list_files(&list_input(None, None)));
  assert_eq!(
    all,
    "README.md\nsrc/main.rs\nsrc/nested/lib.rs\ntarget/out.txt\n[ripgrep was not found: folders were walked \
     without .gitignore rules]"
  );
  let nested = ok(tools.list_files(&list_input(Some("nested/*.rs"), Some("src"))));
  assert!(nested.starts_with("src/nested/lib.rs\n["), "{nested}");
  let braces = ok(tools.list_files(&list_input(Some("**/*.{md,txt}"), None)));
  assert!(braces.starts_with("README.md\ntarget/out.txt\n["), "{braces}");
  let none = ok(tools.list_files(&list_input(Some("*.py"), None)));
  assert!(none.starts_with("no files match"), "{none}");
}

#[test]
fn files_list_stays_in_the_scope() {
  let layout = Layout::temporary();
  let error = failed(
    layout
      .tools()
      .list_files(&list_input(None, Some(&text(&layout.outside)))),
  );
  assert!(error.contains("outside the folders"), "{error}");
  let file = failed(layout.tools().list_files(&list_input(None, Some("README.md"))));
  assert!(file.contains("is not a folder"), "{file}");
}

#[test]
fn files_glob_without_ripgrep_is_bounded() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let long = "a".repeat(300);
  let error = failed(tools.list_files(&list_input(Some(&long), None)));
  assert!(error.contains("longer than 256 characters"), "{error}");
  let braces = "{a,b}".repeat(7);
  let error = failed(tools.list_files(&list_input(Some(&braces), None)));
  assert!(error.contains("more than 64 alternatives"), "{error}");
  std::fs::write(layout.worktree.join(format!("{}.txt", "a".repeat(200))), "x").expect("file");
  let started = std::time::Instant::now();
  let none = ok(tools.list_files(&list_input(Some("*a*a*a*a*a*a*a*a*a*a*b"), None)));
  assert!(none.starts_with("no files match"), "{none}");
  assert!(started.elapsed() < std::time::Duration::from_secs(5));
}

/// Known files (the code index's list, git's changes) are confined as `read_file` confines a path: entries
/// that leave the worktree, are absolute or hide `..` behind `\` are left out and counted, and a folder outside
/// the worktree is listed as always.
#[test]
fn files_list_known_files_are_confined() {
  let layout = Layout::temporary();
  let tools = layout.tools();
  let outside = text(&layout.root.join("outside").join("secret.txt"));
  let known = [
    "README.md".to_owned(),
    "src/main.rs".to_owned(),
    "../../outside/secret.txt".to_owned(),
    r"x\..\..\..\outside\secret.txt".to_owned(),
    outside,
    "C:secret.txt".to_owned(),
  ];
  let listed = ok(tools.list_known_files(&list_input(None, None), &known));
  assert_eq!(
    listed,
    "README.md\nsrc/main.rs\n[4 entries of the code index or of git were no paths in the worktree and were left \
     out]"
  );
  assert!(!listed.contains("secret"));
  let index = ok(tools.list_known_files(&list_input(None, Some(&text(&layout.index))), &known));
  assert!(index.contains("symbols.tsv"), "{index}");
}
