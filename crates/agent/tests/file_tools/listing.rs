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
