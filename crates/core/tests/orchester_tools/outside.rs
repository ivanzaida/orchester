//! Reads and searches outside the worktree: refused credential locations and Orchester's own state, numbered lines,
//! a file read as it comes, never held whole, and the folder a call reads in.

use std::fs;

use orchester_core::{
  orchester_tools::{MAX_LINE_BYTES, MAX_READ_BYTES, OutsideReadError, OutsideReads, ToolWork},
  permission::category::CommandScope,
};
use serde_json::json;

use crate::{Folders, context, parse, text};

#[test]
fn orchester_tools_read_outside_refuses_credentials_and_protected_folders() {
  let folders = Folders::new();
  let reads = folders.reads();
  for path in [
    folders.home.join(".ssh").join("id_ed25519"),
    folders
      .home
      .join("AppData")
      .join("Roaming")
      .join("gh")
      .join("hosts.yml"),
    folders.home.join(".cargo").join("registry").join("lib.rs"),
  ] {
    assert!(
      matches!(reads.read(&text(&path), 1, 10), Err(OutsideReadError::Credentials(_))),
      "{}",
      path.display()
    );
  }
  for path in [
    folders.dependency.join(".git").join("config"),
    folders.private.join("invocation.json"),
  ] {
    assert!(
      matches!(reads.read(&text(&path), 1, 10), Err(OutsideReadError::Protected(_))),
      "{}",
      path.display()
    );
  }
  // A refused folder that holds the worktree, as the repository's `.orchester` does, still lets it be read.
  fs::write(
    folders.worktree.join("notes.md"),
    "in the worktree
",
  )
  .expect("a file");
  let around = folders.worktree.parent().expect("a parent").to_path_buf();
  let scope = CommandScope::new(&folders.worktree, &[], Some(&folders.home), None);
  let holding = OutsideReads::new(scope, &[around]);
  assert!(holding.read(&text(&folders.worktree.join("notes.md")), 1, 10).is_ok());
  assert!(matches!(
    holding.read(&text(&folders.dependency.join("src").join("lib.rs")), 1, 10),
    Err(OutsideReadError::Protected(_))
  ));
  assert!(matches!(
    reads.read("deps/src/lib.rs", 1, 10),
    Err(OutsideReadError::NotAbsolute(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency), 1, 10),
    Err(OutsideReadError::NotAFile(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency.join("blob.bin")), 1, 10),
    Err(OutsideReadError::Binary(_))
  ));
  assert!(matches!(
    reads.read(&text(&folders.dependency.join("missing.rs")), 1, 10),
    Err(OutsideReadError::Path(_))
  ));
}
#[test]
fn orchester_tools_read_outside_returns_numbered_lines_and_where_to_go_on() {
  let folders = Folders::new();
  let reads = folders.reads();
  let file = text(&folders.dependency.join("src").join("lib.rs"));
  assert_eq!(
    reads.read(&file, 2, 2).expect("readable"),
    "     2\tline 2\n     3\tline 3\n[the file goes on: read on from start_line 4]\n"
  );
  assert_eq!(reads.read(&file, 4, 10).expect("readable"), "     4\tline 4\n");
  let context = context(&folders.worktree, &reads, &[]);
  let call = parse("read_external_file", &json!({"filename": file, "start_line": 3})).expect("valid");
  let Ok(ToolWork::Done(result)) = call.work(&context) else {
    panic!("a read is answered at once");
  };
  assert!(!result.is_error());
  assert_eq!(result.text(), "     3\tline 3\n     4\tline 4\n");
}

/// Every project's private files, every `.orchester` folder and an application database are Orchester's
/// own state, wherever they lie; the run's worktree inside its repository's `.orchester` stays readable.
#[test]
fn orchester_tools_read_outside_refuses_orchester_state() {
  let folders = Folders::new();
  let other = folders.dependency.join(".orchester").join("orchester.db");
  let installed = folders.dependency.join("install").join("orchester.db-wal");
  let server = folders.dependency.join("server").join("orchester-server.db");
  let private = folders.private.join("other-project").join("inv-approvals.json");
  for path in [&other, &installed, &server, &private] {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
    fs::write(path, "secret").expect("a file");
  }
  let reads = folders.reads();
  for path in [&other, &installed, &server, &private] {
    assert!(
      matches!(reads.read(&text(path), 1, 10), Err(OutsideReadError::Protected(_))),
      "{}",
      path.display()
    );
  }
  let worktree = folders.dependency.join(".orchester").join("worktrees").join("run");
  fs::create_dir_all(&worktree).expect("a worktree");
  fs::write(worktree.join("notes.md"), "mine\n").expect("a file");
  let scope = CommandScope::new(&worktree, &[], Some(&folders.home), None);
  let reads = OutsideReads::new(scope, &[]);
  assert_eq!(
    reads.read(&text(&worktree.join("notes.md")), 1, 10),
    Ok("     1\tmine\n".to_owned())
  );
}

/// A file of several megabytes is read as it comes: one long line is cut to its cap, and many lines stop
/// at the read's cap, also when the read starts far into the file.
#[test]
fn orchester_tools_read_outside_stops_at_its_caps() {
  let folders = Folders::new();
  let reads = folders.reads();
  let one_line = folders.dependency.join("minified.js");
  fs::write(&one_line, "x".repeat(5 * 1024 * 1024)).expect("a file");
  let read = reads.read(&text(&one_line), 1, 10).expect("readable");
  assert!(read.len() < MAX_LINE_BYTES + 100, "{}", read.len());
  assert!(read.ends_with("[rest of the line left out]\n"));
  let many = folders.dependency.join("log.txt");
  fs::write(&many, format!("{}\n", "z".repeat(99)).repeat(60_000)).expect("a file");
  let read = reads.read(&text(&many), 2, 10_000).expect("readable");
  assert!(read.len() <= MAX_READ_BYTES + 30_000, "{}", read.len());
  assert!(
    read.ends_with("[the file goes on: read on from start_line 575]\n"),
    "{}",
    &read[read.len() - 80..]
  );
  let call = parse(
    "read_external_file",
    &json!({"filename": text(&many), "start_line": 60_000, "length": 1}),
  )
  .expect("valid");
  let context = context(&folders.worktree, &reads, &[]);
  let Ok(ToolWork::Done(result)) = call.work(&context) else {
    panic!("a read is answered at once");
  };
  assert!(result.text().starts_with(" 60000\t"), "{}", result.text());
}

/// The shared credential rule spares published sources in the home folder's dot folders, such as a package
/// registry's sources, which a read outside the worktree then reaches; the registry's own files stay refused.
#[test]
fn orchester_tools_read_outside_follows_the_shared_credential_rule() {
  let folders = Folders::new();
  let source = folders
    .home
    .join(".cargo")
    .join("registry")
    .join("src")
    .join("index.crates.io-0000")
    .join("lurq-0.40.1")
    .join("src")
    .join("lib.rs");
  fs::create_dir_all(source.parent().expect("a parent")).expect("a folder");
  fs::write(&source, "pub fn shown() {}\n").expect("a file");
  let reads = folders.reads();
  assert_eq!(
    reads.read(&text(&source), 1, 10),
    Ok("     1\tpub fn shown() {}\n".to_owned())
  );
  let credentials = folders.home.join(".cargo").join("credentials.toml");
  fs::write(&credentials, "token = \"x\"\n").expect("a file");
  assert!(matches!(
    reads.read(&text(&credentials), 1, 10),
    Err(OutsideReadError::Credentials(_))
  ));
}

/// A NUL makes a file no text file wherever it lies up to the last line read, in a line before the offset too.
#[test]
fn orchester_tools_read_outside_refuses_a_nul_before_the_offset() {
  let folders = Folders::new();
  let reads = folders.reads();
  let file = folders.dependency.join("data.db");
  fs::write(&file, b"SQLite format 3\0\x01\nreadable text\nmore\n").expect("a file");
  assert!(matches!(
    reads.read(&text(&file), 2, 10),
    Err(OutsideReadError::Binary(_))
  ));
}

/// A search outside the worktree names a folder that is checked as a read is; a read's folder is its file's. Both are
/// what an approval shows and Approve similar keeps its rule to.
#[test]
fn orchester_tools_external_calls_name_the_folder_they_read_in() {
  let folders = Folders::new();
  let reads = folders.reads();
  let source = folders.dependency.join("src");
  let resolved = |path: &std::path::Path| fs::canonicalize(path).expect("a folder");
  let search = parse(
    "search_external_file",
    &json!({"base_directory": text(&source), "term": "line"}),
  )
  .expect("valid");
  let folder = search.external_folder(&reads).expect("outside").expect("allowed");
  assert_eq!(resolved(&folder), resolved(&source));
  let context = context(&folders.worktree, &reads, &[]);
  let Ok(ToolWork::SearchExternal { folder, term }) = search.work(&context) else {
    panic!("a search is the caller's to run");
  };
  assert_eq!((resolved(&folder), term.as_str()), (resolved(&source), "line"));
  let read = parse("read_external_file", &json!({"filename": text(&source.join("lib.rs"))})).expect("valid");
  let folder = read.external_folder(&reads).expect("outside").expect("allowed");
  assert_eq!(resolved(&folder), resolved(&source));
  for (base, refused) in [
    (folders.home.join(".ssh"), "credential"),
    (folders.dependency.join(".git"), "protected"),
    (source.join("lib.rs"), "not a folder"),
  ] {
    let call = parse(
      "search_external_file",
      &json!({"base_directory": text(&base), "term": "x"}),
    )
    .expect("valid");
    let error = call.external_folder(&reads).expect("outside").expect_err("refused");
    assert!(error.to_string().contains(refused), "{error}");
    assert!(call.work(&context).is_err());
  }
  let relative = parse("search_external_file", &json!({"base_directory": "deps", "term": "x"})).expect("valid");
  assert!(matches!(
    relative.external_folder(&reads),
    Some(Err(OutsideReadError::NotAbsolute(_)))
  ));
  let worktree = parse("read_file", &json!({"filename": "README.md"})).expect("valid");
  assert!(worktree.external_folder(&reads).is_none());
}
