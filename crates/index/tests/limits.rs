//! Files the index only counts: text over 1 MiB is not kept in memory or parsed, and a source file
//! with a very long line (minified, generated) is not parsed. Both keep their row in `tree.txt`
//! with their line count.

mod support;

use support::{Repo, read};

#[test]
fn large_and_minified_files_are_counted_but_not_parsed() {
  let repo = Repo::new();
  let large: String = (0..60_000).map(|number| format!("pub fn f{number}() {{}}\n")).collect();
  assert!(large.len() > 1024 * 1024);
  repo.write("gen/large.rs", &large);
  let minified = format!("{}\n", "function a(){return 1};".repeat(300));
  repo.write("web/min.js", &minified);
  repo.git(&["add", "gen", "web/min.js"]);
  repo.git(&["commit", "--quiet", "-m", "generated"]);

  let index = repo.ensure(&repo.head()).path;

  let tree = read(&index, "tree.txt");
  assert!(tree.contains("gen/large.rs\trust\t60000\n"), "{tree}");
  assert!(tree.contains("web/min.js\tjavascript\t1\n"), "{tree}");
  let symbols = read(&index, "symbols.tsv");
  assert!(!symbols.contains("gen/large.rs"));
  assert!(!symbols.contains("web/min.js"));
  assert!(!index.join("outline/gen").exists());
  assert!(!index.join("outline/web/min.js.md").exists());
  assert!(index.join("outline/web/src/index.ts.md").is_file());
}
