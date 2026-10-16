//! The code viewer's reads (`docs/design/code-viewer.md`): which paths open and which are refused with
//! their reason, a file and the tree at the checkout, a run's worktree, a commit and the base branch,
//! the versions a file has, binary and too large files, file-name search, search within a file.

use std::{collections::BTreeSet, fs, path::Path, process, sync::Arc};

use orchester_core::{command::CommandRunner, id::RunId, revision::Revision};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  files::{
    EntryKind, FileFacts, FileList, FilePath, FileText, FileView, FilesError, LineRange, MAX_VIEW_BYTES, PathError,
    TextMatch, Version, list_files, read_file, versions,
  },
  tasks::changes::run_worktree,
};
use tempfile::TempDir;

fn git(repo: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository on `main` with `src/main.rs` and `README.md` committed; returns it and the commit.
fn repository() -> (TempDir, String) {
  let dir = tempfile::tempdir().expect("a temporary folder");
  let repo = dir.path();
  git(repo, &["init", "--quiet", "--initial-branch=main"]);
  git(repo, &["config", "user.name", "Orchester Test"]);
  git(repo, &["config", "user.email", "test@orchester.invalid"]);
  git(repo, &["config", "core.autocrlf", "false"]);
  fs::create_dir_all(repo.join("src")).expect("src");
  fs::write(repo.join("src/main.rs"), "fn main() {\n  println!(\"one\");\n}\n").expect("main.rs");
  fs::write(repo.join("README.md"), "# Kontur\n").expect("README");
  fs::write(repo.join(".gitignore"), "target/\n.orchester/\n").expect(".gitignore");
  git(repo, &["add", "."]);
  git(repo, &["commit", "--quiet", "-m", "first"]);
  let head = git(repo, &["rev-parse", "HEAD"]);
  (dir, head)
}

fn runner() -> Programs {
  Programs::processes()
}

fn path(value: &str) -> FilePath {
  FilePath::new(value).expect("a project path")
}

fn read(repo: &Path, version: &Version, file: &str) -> Result<FileView, FilesError> {
  let programs = runner();
  read_file(programs.runner() as &dyn CommandRunner, repo, version, &path(file))
}

fn lines(view: &FileView) -> Vec<String> {
  match view {
    FileView::Text(text) => text.lines().iter().map(ToString::to_string).collect(),
    other => panic!("not text: {other:?}"),
  }
}

#[test]
fn files_paths_that_step_out_or_hide_are_refused_with_their_reason() {
  assert_eq!(FilePath::new(""), Err(PathError::Empty));
  assert_eq!(FilePath::new("./"), Err(PathError::Empty));
  assert!(matches!(FilePath::new("../secret.txt"), Err(PathError::Parent(_))));
  assert!(matches!(FilePath::new("src/../../x"), Err(PathError::Parent(_))));
  assert!(matches!(FilePath::new("/etc/passwd"), Err(PathError::Absolute(_))));
  assert!(matches!(
    FilePath::new("C:/Windows/win.ini"),
    Err(PathError::Absolute(_))
  ));
  assert!(matches!(FilePath::new("c:secret"), Err(PathError::Absolute(_))));
  assert!(matches!(FilePath::new("src\\..\\..\\x"), Err(PathError::Invalid(_))));
  assert!(matches!(FilePath::new(".git/config"), Err(PathError::Hidden(_))));
  assert!(matches!(FilePath::new("vendor/.GIT/HEAD"), Err(PathError::Hidden(_))));
  assert!(matches!(
    FilePath::new(".orchester/project.db"),
    Err(PathError::Hidden(_))
  ));
  assert_eq!(path("docs/.orchester/notes.md").as_str(), "docs/.orchester/notes.md");
  assert_eq!(path("./src//main.rs").as_str(), "src/main.rs");
  assert_eq!(path("src/a/b.rs").folders().collect::<Vec<_>>(), ["src", "src/a"]);
  assert_eq!(path("src/a/b.rs").name(), "b.rs");
}

#[test]
fn files_versions_and_line_ranges_read_back_from_their_route_values() {
  let run = RunId::generate();
  let revision = "0123456789abcdef0123456789abcdef01234567";
  for version in [
    Version::Checkout,
    Version::Base,
    Version::Run(run),
    Version::Revision(Revision::new(revision).expect("a revision")),
  ] {
    assert_eq!(version.to_string().parse::<Version>(), Ok(version.clone()));
  }
  assert_eq!(format!("run:{run}").parse::<Version>(), Ok(Version::Run(run)));
  assert!("rev:main".parse::<Version>().is_err());
  assert!("worktree".parse::<Version>().is_err());
  let range: LineRange = "12-20".parse().expect("a range");
  assert_eq!((range.first(), range.last()), (12, 20));
  assert_eq!("20-12".parse::<LineRange>(), Ok(range));
  assert_eq!(
    "7".parse::<LineRange>().map(|line| line.to_string()),
    Ok("7".to_owned())
  );
  assert!("0-3".parse::<LineRange>().is_err());
  assert!("a-b".parse::<LineRange>().is_err());
}

#[test]
fn files_text_copies_lines_and_finds_text_without_regard_to_case() {
  let text = FileText::parse(b"\xEF\xBB\xBFfirst Line\r\n\tsecond line\nthird\n");
  assert_eq!(text.lines().len(), 3);
  assert_eq!(&*text.lines()[0], "first Line");
  let range = LineRange::new(2, 9).expect("a range");
  assert_eq!(text.copied(range), "\tsecond line\nthird");
  let find = |query: &str, match_case| text.find(query, match_case, &|| true).expect("wanted");
  let found = find("LINE", false);
  assert_eq!(found.len(), 2);
  assert_eq!((found[0].line, found[0].start, found[0].end), (0, 6, 10));
  // The tab shows as four spaces, so the match starts after them.
  assert_eq!((found[1].line, found[1].start), (1, 11));
  assert!(find("", false).is_empty());
  let place = |line, start| TextMatch {
    line,
    start,
    end: start + 1,
  };
  let found = [place(1, 0), place(4, 2), place(9, 0)];
  assert_eq!(TextMatch::same_place(&found, place(4, 2)), Some(1), "the same place");
  assert_eq!(
    TextMatch::same_place(&found, place(5, 0)),
    Some(2),
    "the next place after it"
  );
  assert_eq!(
    TextMatch::same_place(&found, place(12, 0)),
    Some(2),
    "the last when none is after it"
  );
  assert_eq!(TextMatch::same_place(&[], place(1, 0)), None);
  let cased = find("Line", true);
  assert_eq!(cased.len(), 1, "only the line that has it in its case");
  assert_eq!((cased[0].line, cased[0].start), (0, 6));
  assert!(find("LINE", true).is_empty());
  assert!(FileText::parse(b"").lines().is_empty());
  assert_eq!(text.find("line", false, &|| false), None, "superseded, it stops");
}

#[test]
fn files_digests_tell_a_text_and_a_list_apart_by_content_and_size() {
  let digest = |bytes: &[u8]| FileText::parse(bytes).digest();
  assert_eq!(digest(b"a\nb\n"), digest(b"a\nb\n"));
  assert_ne!(digest(b"a\nb\n"), digest(b"ab\n"), "lines do not run into each other");
  assert_ne!(digest(b"a\nb\n"), digest(b"a\r\nb\n"), "the same lines in more bytes");
  let list = |paths: &[&str]| FileList::new(paths.iter().map(|path| (*path).to_owned())).digest();
  assert_eq!(list(&["a.rs", "b.rs"]), list(&["b.rs", "a.rs"]));
  assert_ne!(list(&["a.rs", "b.rs"]), list(&["a.rsb.rs"]));
  assert_ne!(
    FileView::Binary { size: 0 }.digest(),
    FileView::Text(Arc::new(FileText::parse(b""))).digest(),
    "a binary file and an empty text of its size"
  );
  assert_ne!(
    FileView::Binary { size: 9 }.digest(),
    FileView::TooLarge { size: 9 }.digest()
  );
}

#[test]
fn files_tree_shows_folders_first_and_opens_only_open_folders() {
  let list = FileList::new(
    [
      "src/main.rs",
      "README.md",
      "src/ui/row.rs",
      "Cargo.toml",
      ".git/HEAD",
      ".orchester/x",
      "docs/a.md",
    ]
    .map(String::from),
  );
  assert_eq!(list.len(), 5);
  let top: Vec<(String, EntryKind)> = list
    .rows(&BTreeSet::new())
    .iter()
    .map(|row| (row.name.to_string(), row.kind))
    .collect();
  let closed = EntryKind::Folder { open: false };
  assert_eq!(
    top,
    [
      ("docs".to_owned(), closed),
      ("src".to_owned(), closed),
      ("Cargo.toml".to_owned(), EntryKind::File),
      ("README.md".to_owned(), EntryKind::File),
    ]
  );
  let open: BTreeSet<Arc<str>> = [Arc::from("src")].into();
  let rows = list.rows(&open);
  let shown: Vec<(&str, usize)> = rows.iter().map(|row| (&*row.path, row.depth)).collect();
  assert_eq!(
    shown,
    [
      ("docs", 0),
      ("src", 0),
      ("src/ui", 1),
      ("src/main.rs", 1),
      ("Cargo.toml", 0),
      ("README.md", 0)
    ]
  );
  assert!(list.contains("src/ui/row.rs"));
  assert!(!list.contains("src/ui"));
}

#[test]
fn files_name_search_puts_the_file_name_before_the_path() {
  let list = FileList::new(
    [
      "src/main.rs",
      "docs/maintenance.md",
      "src/domain/mod.rs",
      "crates/app/src/main.rs",
      "README.md",
    ]
    .map(String::from),
  );
  let found: Vec<String> = list.search("main", 10).paths.iter().map(ToString::to_string).collect();
  assert_eq!(
    found,
    [
      "src/main.rs",
      "crates/app/src/main.rs",
      "docs/maintenance.md",
      "src/domain/mod.rs"
    ],
    "a name that is the query but for its extension first"
  );
  let found: Vec<String> = list.search("SDM", 10).paths.iter().map(ToString::to_string).collect();
  assert_eq!(found, ["src/domain/mod.rs"]);
  let all = list.search("   ", 3);
  assert_eq!(
    (all.paths.len(), all.total),
    (3, 5),
    "with nothing typed, the version's files"
  );
  let some = list.search("s", 2);
  assert_eq!((some.paths.len(), some.total), (2, 4));
}

#[test]
fn files_name_search_puts_the_name_closest_to_the_query_first() {
  let list = FileList::new(
    [
      "plan_limits_1.rs",
      "plan_limits_2.rs",
      "crates/workspace/src/plan_limits.rs",
      "crates/core/src/connection/plan_limits/parse.rs",
    ]
    .map(String::from),
  );
  let found: Vec<String> = list
    .search("plan_lim", 10)
    .paths
    .iter()
    .map(ToString::to_string)
    .collect();
  assert_eq!(
    found,
    [
      "crates/workspace/src/plan_limits.rs",
      "plan_limits_1.rs",
      "plan_limits_2.rs",
      "crates/core/src/connection/plan_limits/parse.rs"
    ]
  );
}

#[test]
fn files_checkout_reads_text_and_explains_binary_large_missing_and_folders() {
  let (dir, _) = repository();
  let repo = dir.path();
  let checkout = Version::Checkout;
  assert_eq!(
    lines(&read(repo, &checkout, "src/main.rs").expect("main.rs")),
    ["fn main() {", "  println!(\"one\");", "}"]
  );
  fs::write(repo.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).expect("png");
  assert_eq!(read(repo, &checkout, "logo.png"), Ok(FileView::Binary { size: 7 }));
  let large = usize::try_from(MAX_VIEW_BYTES).expect("fits") + 1;
  fs::write(repo.join("large.txt"), "x".repeat(large)).expect("large");
  assert_eq!(
    read(repo, &checkout, "large.txt"),
    Ok(FileView::TooLarge {
      size: MAX_VIEW_BYTES + 1
    })
  );
  assert_eq!(
    read(repo, &checkout, "gone.rs"),
    Err(FilesError::Missing(path("gone.rs")))
  );
  assert_eq!(read(repo, &checkout, "src"), Err(FilesError::Folder(path("src"))));
  assert_eq!(
    FilesError::Missing(path("gone.rs")).message().key,
    "files.failure.missing"
  );
}

#[test]
fn files_a_link_or_junction_that_leads_out_is_refused() {
  let (dir, _) = repository();
  let outside = tempfile::tempdir().expect("an outside folder");
  fs::write(outside.path().join("secret.txt"), "secret").expect("secret");
  link_folder(outside.path(), &dir.path().join("escape"));
  let refused = read(dir.path(), &Version::Checkout, "escape/secret.txt");
  assert_eq!(
    refused,
    Err(FilesError::Path(PathError::Outside("escape/secret.txt".to_owned())))
  );
  assert_eq!(refused.expect_err("refused").message().key, "files.refused.outside");
  link_folder(&dir.path().join("src"), &dir.path().join("inside"));
  assert!(read(dir.path(), &Version::Checkout, "inside/main.rs").is_ok());
}

/// `link` as a link to the folder `target`: a junction on Windows, which needs no privilege; a symbolic link elsewhere.
#[cfg(windows)]
fn link_folder(target: &Path, link: &Path) {
  let status = process::Command::new("cmd")
    .args(["/C", "mklink", "/J"])
    .arg(link)
    .arg(target)
    .output()
    .expect("run mklink");
  assert!(
    status.status.success(),
    "mklink: {}",
    String::from_utf8_lossy(&status.stderr)
  );
}

#[cfg(not(windows))]
fn link_folder(target: &Path, link: &Path) {
  std::os::unix::fs::symlink(target, link).expect("a symbolic link");
}

#[test]
fn files_listing_holds_untracked_files_and_leaves_out_ignored_deleted_and_hidden_ones() {
  let (dir, _) = repository();
  let repo = dir.path();
  fs::write(repo.join("notes.md"), "draft\n").expect("notes");
  fs::create_dir_all(repo.join("target")).expect("target");
  fs::write(repo.join("target/out.bin"), "built").expect("out");
  fs::create_dir_all(repo.join(".orchester")).expect(".orchester");
  fs::write(repo.join(".orchester/state"), "x").expect("state");
  fs::remove_file(repo.join("README.md")).expect("remove README");
  let programs = runner();
  let list = list_files(programs.runner(), repo, &Version::Checkout).expect("the checkout's files");
  let rows: Vec<String> = list
    .rows(&[Arc::from("src")].into())
    .iter()
    .map(|row| row.path.to_string())
    .collect();
  assert_eq!(rows, ["src", "src/main.rs", ".gitignore", "notes.md"]);
}

#[test]
fn files_a_commit_and_the_base_branch_read_what_git_holds() {
  let (dir, first) = repository();
  let repo = dir.path();
  fs::write(repo.join("src/main.rs"), "fn main() {}\n").expect("edit");
  let at_first = Version::Revision(Revision::new(&first).expect("a commit"));
  assert_eq!(lines(&read(repo, &at_first, "src/main.rs").expect("at first")).len(), 3);
  assert_eq!(
    lines(&read(repo, &Version::Base, "src/main.rs").expect("at base")).len(),
    3
  );
  assert_eq!(
    lines(&read(repo, &Version::Checkout, "src/main.rs").expect("checkout")),
    ["fn main() {}"]
  );
  assert_eq!(read(repo, &at_first, "src"), Err(FilesError::Folder(path("src"))));
  assert_eq!(
    read(repo, &at_first, "notes.md"),
    Err(FilesError::Missing(path("notes.md")))
  );
  assert_eq!(
    read(repo, &at_first, "src/*.rs"),
    Err(FilesError::Missing(path("src/*.rs")))
  );
  let programs = runner();
  let list = list_files(programs.runner(), repo, &at_first).expect("the commit's files");
  assert!(list.contains("src/main.rs") && list.contains("README.md"));
  let lost = Version::Revision(Revision::new("0123456789abcdef0123456789abcdef01234567").expect("a commit"));
  assert_eq!(
    read(repo, &lost, "README.md"),
    Err(FilesError::VersionGone(lost.clone()))
  );
  git(repo, &["checkout", "--quiet", "--detach"]);
  assert_eq!(
    read(repo, &Version::Base, "README.md"),
    Err(FilesError::VersionGone(Version::Base))
  );
}

/// A version as the switch lists it: its branch, its commit and what the file is there.
type Listed = (Version, Option<String>, Option<String>, Option<FileFacts>);

#[test]
fn files_a_runs_worktree_is_a_version_while_it_exists() {
  let (dir, first) = repository();
  let repo = dir.path();
  let run = RunId::generate();
  let worktree = run_worktree(repo, run);
  let shown = worktree.to_string_lossy().into_owned();
  git(
    repo,
    &["worktree", "add", "--quiet", "-b", "orchester/run", &shown, "HEAD"],
  );
  fs::write(worktree.join("src/main.rs"), "fn main() { run(); }\n").expect("the run's edit");
  let in_run = Version::Run(run);
  assert_eq!(
    lines(&read(repo, &in_run, "src/main.rs").expect("in the run")),
    ["fn main() { run(); }"]
  );
  let programs = runner();
  let at_first = Version::Revision(Revision::new(&first).expect("a commit"));
  let choices = versions(
    programs.runner(),
    repo,
    (&at_first, Some(run)),
    Some(&path("README.md")),
  )
  .expect("versions");
  let readme = Some(FileFacts::Text { lines: 1, size: 9 });
  assert_eq!(choices.run, None, "the fixture has no project database to name the run");
  let listed: Vec<Listed> = choices
    .choices
    .iter()
    .map(|choice| {
      (
        choice.version.clone(),
        choice.branch.as_ref().map(ToString::to_string),
        choice.commit.as_ref().map(ToString::to_string),
        choice.file,
      )
    })
    .collect();
  assert_eq!(
    listed,
    [
      (Version::Checkout, Some("main".to_owned()), None, readme),
      (in_run.clone(), None, None, readme),
      (Version::Base, Some("main".to_owned()), Some(first.clone()), readme),
      (at_first, None, Some(first), readme),
    ]
  );
  let choices = versions(
    programs.runner(),
    repo,
    (&Version::Checkout, None),
    Some(&path("gone.md")),
  )
  .expect("versions");
  assert_eq!(choices.choices[0].file, Some(FileFacts::Absent));
  git(repo, &["worktree", "remove", "--force", &shown]);
  assert_eq!(
    read(repo, &in_run, "README.md"),
    Err(FilesError::VersionGone(in_run.clone()))
  );
  let choices = versions(programs.runner(), repo, (&Version::Checkout, Some(run)), None).expect("versions");
  let listed: Vec<&Version> = choices.choices.iter().map(|choice| &choice.version).collect();
  assert_eq!(listed, [&Version::Checkout, &Version::Base]);
  assert_eq!(
    FilesError::VersionGone(in_run).message().key,
    "files.failure.worktreeGone"
  );
}
