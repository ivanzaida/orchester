//! Where the native file tools may read and write: the roots, protected folders, links and command
//! arguments, on every platform.

use std::fs;

use orchester_core::{native::scope::ScopeError, permission::RepositoryAccess};

use crate::support::{Layout, text};

#[test]
fn native_scope_worktree_paths() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let main = layout.worktree.join("src/main.rs");
  assert_eq!(scope.readable("src/main.rs"), Ok(main.clone()));
  assert_eq!(scope.readable("src/../src/./main.rs"), Ok(main.clone()));
  assert_eq!(scope.readable(&text(&main)), Ok(main.clone()));
  assert_eq!(scope.writable("src/main.rs"), Ok(main));
  assert_eq!(scope.worktree(), layout.worktree);
  assert_eq!(
    scope.writable("src/new/deeper/file.rs"),
    Ok(layout.worktree.join("src/new/deeper/file.rs")),
    "missing folders are created by the executor"
  );
}

#[test]
fn native_scope_refuses_outside() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let secret = layout.outside.join("secret.txt");
  assert_eq!(
    scope.readable(&text(&secret)),
    Err(ScopeError::NotReadable(secret.clone()))
  );
  assert_eq!(
    scope.writable(&text(&secret)),
    Err(ScopeError::NotWritable(secret.clone()))
  );
  assert_eq!(
    scope.readable("../../../../outside/secret.txt"),
    Err(ScopeError::NotReadable(secret))
  );
  assert_eq!(scope.readable("a\0b"), Err(ScopeError::Nul));
  assert!(matches!(scope.readable("../../../.."), Err(ScopeError::NotReadable(_))));
}

#[test]
fn native_scope_read_only_roots() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let map = layout.index.join("map.md");
  assert_eq!(scope.readable(&text(&map)), Ok(map.clone()));
  assert_eq!(scope.writable(&text(&map)), Err(ScopeError::Protected(map)));
  let skill = layout.skills.join("new.md");
  assert_eq!(scope.writable(&text(&skill)), Err(ScopeError::Protected(skill)));
}

#[test]
fn native_scope_read_only_access() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::ReadOnly);
  let main = layout.worktree.join("src/main.rs");
  assert_eq!(scope.readable("src/main.rs"), Ok(main.clone()));
  assert_eq!(scope.writable("src/main.rs"), Err(ScopeError::NotWritable(main)));
  let cached = layout.cache.join("a.txt");
  assert_eq!(
    scope.writable(&text(&cached)),
    Err(ScopeError::NotWritable(cached)),
    "writable folders apply only when the step edits files"
  );
}

#[test]
fn native_scope_writable_folders() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let cached = layout.cache.join("target/debug/out");
  assert_eq!(scope.readable(&text(&cached)), Ok(cached.clone()));
  assert_eq!(scope.writable(&text(&cached)), Ok(cached));
  let git = layout.cache.join("dep/.git/config");
  assert_eq!(scope.writable(&text(&git)), Err(ScopeError::Protected(git)));
}

#[test]
fn native_scope_protected_folders() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  for path in [
    ".git",
    ".GIT",
    "vendor/lib/.git/hooks/pre-commit",
    "vendor/new/.git/config",
    ".orchester/notes.md",
  ] {
    assert!(matches!(scope.writable(path), Err(ScopeError::Protected(_))), "{path}");
  }
  assert!(scope.readable(".git").is_ok(), "reading .git is allowed");
  assert!(
    scope.writable("src/.orchester/notes.md").is_ok(),
    "only the top-level .orchester is protected"
  );
}

#[test]
fn native_scope_hard_link_is_its_entry() {
  let layout = Layout::new();
  let secret = layout.outside.join("secret.txt");
  let link = layout.worktree.join("src/hard.txt");
  fs::hard_link(&secret, &link).expect("hard link");
  let scope = layout.scope(RepositoryAccess::Write);
  assert_eq!(scope.readable("src/hard.txt"), Ok(link.clone()));
  let target = scope.writable("src/hard.txt").expect("writable");
  assert_eq!(target, link);

  // The executor writes a new file and renames it over the entry (§4.4): the outside file keeps its text.
  let temporary = layout.worktree.join("src/hard.txt.tmp");
  fs::write(&temporary, "replaced\n").expect("temporary");
  fs::rename(&temporary, &target).expect("rename");
  assert_eq!(fs::read_to_string(&secret).expect("secret"), "secret\n");
  assert_eq!(fs::read_to_string(&link).expect("link"), "replaced\n");
}

#[test]
fn native_scope_workdir() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::ReadOnly);
  assert_eq!(scope.workdir(None), Ok(layout.worktree.clone()));
  assert_eq!(scope.workdir(Some("src")), Ok(layout.worktree.join("src")));
  assert!(
    scope.workdir(Some(&text(&layout.cache))).is_err(),
    "commands run inside the worktree"
  );
  assert!(scope.workdir(Some("../..")).is_err());
}

#[test]
fn native_scope_command_arguments() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::ReadOnly);
  let src = layout.worktree.join("src");
  assert!(scope.argument_readable(&src, "main.rs"));
  assert!(scope.argument_readable(&src, "--oneline"), "not a path");
  assert!(scope.argument_readable(&src, &text(&layout.index.join("map.md"))));
  assert!(!scope.argument_readable(&src, &text(&layout.outside.join("secret.txt"))));
  assert!(!scope.argument_readable(&src, "../../../../../outside"));
  assert!(!scope.argument_readable(&src, "a\0b"));
}

#[cfg(unix)]
#[test]
fn native_scope_symlinks() {
  use std::os::unix::fs::symlink;

  let layout = Layout::new();
  symlink(&layout.outside, layout.worktree.join("out")).expect("folder link");
  symlink(layout.outside.join("secret.txt"), layout.worktree.join("secret")).expect("file link");
  symlink(layout.worktree.join("src"), layout.worktree.join("source")).expect("inner link");
  let scope = layout.scope(RepositoryAccess::Write);
  let secret = layout.outside.join("secret.txt");
  assert_eq!(
    scope.readable("out/secret.txt"),
    Err(ScopeError::NotReadable(secret.clone()))
  );
  assert_eq!(
    scope.writable("out/new.txt"),
    Err(ScopeError::NotWritable(layout.outside.join("new.txt")))
  );
  assert_eq!(scope.readable("secret"), Err(ScopeError::NotReadable(secret)));
  assert_eq!(
    scope.readable("source/main.rs"),
    Ok(layout.worktree.join("src/main.rs"))
  );
  let src = layout.worktree.join("src");
  assert!(!scope.argument_readable(&src, "../out/secret.txt"));
}

#[cfg(unix)]
#[test]
fn native_scope_unix_names_are_plain() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  for name in ["CON", "nul.txt", "a:b", "COM1"] {
    assert_eq!(scope.writable(name), Ok(layout.worktree.join(name)), "{name}");
  }
}

#[test]
fn native_scope_repository_discovery_excludes_state_and_other_readable_roots() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  assert_eq!(
    scope.repository_readable("src/main.rs"),
    Ok(layout.worktree.join("src/main.rs"))
  );
  for path in [".git", "vendor/lib/.git/config", ".orchester/private.md"] {
    assert!(
      matches!(scope.repository_readable(path), Err(ScopeError::Protected(_))),
      "{path}"
    );
  }
  for path in [
    layout.index.join("map.md"),
    layout.skills.join("SKILL.md"),
    layout.cache.join("file.md"),
  ] {
    assert!(
      scope.readable(&text(&path)).is_ok(),
      "a native agent may read its granted root"
    );
    assert!(matches!(
      scope.repository_readable(&text(&path)),
      Err(ScopeError::NotReadable(_))
    ));
  }
}
