//! Tracked paths that are no plain file on every platform: they stay in the tree and the symbols but
//! get no outline file, and nothing is ever written outside the index folder. The commit is made
//! with `git fast-import`, which takes names the file system of a checkout would refuse.

mod support;

use std::{
  collections::BTreeSet,
  io::Write,
  path::Path,
  process::{Command, Stdio},
};

use orchester_core::revision::Revision;
use support::{Repo, files, read};

const SOURCE: &str = "pub fn f() {}\n";
const PATHS: [&str; 10] = [
  "src/a:b.rs",
  "src/q?.rs",
  r"x\..\..\..\escaped.rs",
  "Foo.rs",
  "foo.rs",
  "CON.rs",
  "com1.rs",
  "dot./x.rs",
  "space /x.rs",
  "ok.rs",
];

/// Commits `paths`, each holding [`SOURCE`], as the only commit of `branch`; its revision.
fn commit_paths(repo: &Repo, branch: &str, paths: &[&str]) -> Revision {
  let mut stream = format!("blob\nmark :1\ndata {}\n{SOURCE}\n", SOURCE.len());
  stream.push_str(&format!(
    "commit refs/heads/{branch}\ncommitter a <a@b.invalid> 0 +0000\ndata 1\nA\n"
  ));
  for path in paths {
    stream.push_str(&format!("M 100644 :1 {path}\n"));
  }
  stream.push('\n');
  let mut import = Command::new("git")
    .arg("-C")
    .arg(&repo.root)
    .args(["-c", "core.ignorecase=false", "fast-import", "--quiet"])
    .stdin(Stdio::piped())
    .spawn()
    .expect("run git fast-import");
  import
    .stdin
    .take()
    .expect("piped")
    .write_all(stream.as_bytes())
    .expect("write the stream");
  assert!(import.wait().expect("wait").success());
  Revision::new(&repo.git(&["rev-parse", branch])).expect("a commit id")
}

fn names_under(folder: &Path) -> BTreeSet<String> {
  walk(folder)
    .into_iter()
    .map(|path| {
      path
        .strip_prefix(folder)
        .expect("inside")
        .to_string_lossy()
        .replace('\\', "/")
    })
    .collect()
}

fn walk(folder: &Path) -> Vec<std::path::PathBuf> {
  let mut found = Vec::new();
  for entry in std::fs::read_dir(folder).expect("read folder") {
    let path = entry.expect("entry").path();
    if path.is_dir() {
      found.extend(walk(&path));
    } else {
      found.push(path);
    }
  }
  found
}

#[test]
fn paths_that_are_no_plain_file_everywhere_get_no_outline_and_never_escape_the_index() {
  let repo = Repo::new();
  let revision = commit_paths(&repo, "odd", &PATHS);
  let before = names_under(&repo.root);

  let index = repo.ensure(&revision).path;

  let outlines = names_under(&index.join("outline"));
  assert_eq!(outlines, BTreeSet::from(["ok.rs.md".to_owned()]));
  let after = names_under(&repo.root);
  let written: Vec<&String> = after
    .difference(&before)
    .filter(|name| !name.starts_with(".orchester/"))
    .collect();
  assert!(written.is_empty(), "written outside the index: {written:?}");
  let tree = read(&index, "tree.txt");
  assert_eq!(tree.lines().count(), PATHS.len(), "{tree}");
  let symbols = read(&index, "symbols.tsv");
  for path in PATHS {
    assert!(
      symbols.contains(&format!("\t{path}\t1\t")),
      "{path} has its symbol:\n{symbols}"
    );
  }
}

#[test]
fn an_incremental_build_outlines_a_file_once_its_case_twin_is_gone() {
  let repo = Repo::new();
  let first = commit_paths(&repo, "twins", &["Foo.rs", "foo.rs", "ok.rs"]);
  repo.ensure(&first);
  let second = commit_paths(&repo, "single", &["Foo.rs", "ok.rs"]);

  let incremental = repo.ensure(&second);

  assert!(incremental.path.join("outline/Foo.rs.md").is_file());
  let incremental_files = files(&incremental.path);
  std::fs::remove_dir_all(repo.index_root()).expect("remove the indexes");
  let full = repo.ensure(&second);
  assert_eq!(files(&full.path), incremental_files);
}
