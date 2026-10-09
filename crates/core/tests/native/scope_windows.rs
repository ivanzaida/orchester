//! The native file scope on Windows: junctions, device names, alternate data streams, verbatim, device,
//! drive-relative and network paths, and names compared as Windows opens them.

use std::process::Command;

use orchester_core::{native::scope::ScopeError, permission::RepositoryAccess};

use crate::support::{Layout, text};

#[test]
fn native_scope_windows_junctions() {
  let layout = Layout::new();
  let junction = layout.worktree.join("out");
  // cmd reads a `/` as the start of a switch.
  let status = Command::new("cmd")
    .args(["/c", "mklink", "/J"])
    .arg(text(&junction).replace('/', "\\"))
    .arg(text(&layout.outside).replace('/', "\\"))
    .output()
    .expect("mklink");
  assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
  let scope = layout.scope(RepositoryAccess::Write);
  let secret = layout.outside.join("secret.txt");
  assert_eq!(scope.readable("out/secret.txt"), Err(ScopeError::NotReadable(secret)));
  assert_eq!(
    scope.writable("out/new.txt"),
    Err(ScopeError::NotWritable(layout.outside.join("new.txt")))
  );
  assert!(!scope.argument_readable(&layout.worktree, r"out\secret.txt"));
}

#[test]
fn native_scope_windows_device_names() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  for path in [
    "CON",
    "nul",
    "src/aux.rs",
    "NUL.txt",
    "com1",
    "LPT9.log",
    "COM¹",
    "con .txt",
    "PRN.",
  ] {
    assert!(matches!(scope.readable(path), Err(ScopeError::DeviceName(_))), "{path}");
    assert!(matches!(scope.writable(path), Err(ScopeError::DeviceName(_))), "{path}");
  }
  for path in ["console.rs", "COM10", "nullable.rs", "src/auxiliary.rs"] {
    assert!(scope.writable(path).is_ok(), "{path}");
  }
}

#[test]
fn native_scope_windows_path_forms() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let verbatim = format!(r"\\?\{}", text(&layout.worktree.join("src/main.rs")));
  assert!(matches!(scope.readable(&verbatim), Err(ScopeError::DevicePath(_))));
  assert!(matches!(
    scope.readable(r"\\.\PhysicalDrive0"),
    Err(ScopeError::DevicePath(_))
  ));
  assert!(matches!(scope.readable("//./pipe/x"), Err(ScopeError::DevicePath(_))));
  assert!(matches!(
    scope.readable("src/main.rs:hidden"),
    Err(ScopeError::AlternateStream(_))
  ));
  assert!(matches!(
    scope.writable("src/main.rs::$DATA"),
    Err(ScopeError::AlternateStream(_))
  ));
  assert!(matches!(scope.readable("C:main.rs"), Err(ScopeError::DriveRelative(_))));
  assert!(matches!(
    scope.readable(r"\\server\share\file.txt"),
    Err(ScopeError::NetworkPath(_))
  ));
  assert!(matches!(
    scope.writable("//server/share/file.txt"),
    Err(ScopeError::NetworkPath(_))
  ));
  assert!(!scope.argument_readable(&layout.worktree, r"\\server\share\file.txt"));
  assert!(
    !scope.argument_readable(&layout.worktree, "C:main.rs"),
    "a command argument is refused drive-relative, as the file tools refuse it"
  );
}

#[test]
fn native_scope_windows_names_without_case() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  let main = layout.worktree.join("src/main.rs");
  assert!(scope.readable("SRC/MAIN.RS").is_ok());
  assert!(scope.readable(&text(&main).to_uppercase()).is_ok());
  for path in [".git.", ".GIT ", "vendor/lib/.Git./hooks/x", ".orchester./x"] {
    assert!(matches!(scope.writable(path), Err(ScopeError::Protected(_))), "{path}");
  }
}

/// Other spellings NTFS opens as a protected folder: a stream of it (`.git::$INDEX_ALLOCATION`) and, on a
/// volume that keeps them, its short name (`GIT~1`), which resolving turns into the long one.
#[test]
fn native_scope_windows_aliases_of_protected_folders() {
  let layout = Layout::new();
  let scope = layout.scope(RepositoryAccess::Write);
  for path in [
    "vendor/lib/.git::$INDEX_ALLOCATION/hooks/x",
    ".orchester::$INDEX_ALLOCATION/x",
  ] {
    assert!(scope.writable(path).is_err(), "{path}");
  }
  let short = layout.worktree.join("vendor/lib/GIT~1");
  if std::fs::symlink_metadata(&short).is_ok() {
    assert!(
      matches!(
        scope.writable("vendor/lib/GIT~1/hooks/x"),
        Err(ScopeError::Protected(_))
      ),
      "the short name of .git"
    );
  }
}

/// A link to a network path is read, never followed: the path through it is refused before anything resolves it, so
/// the server it names is not contacted (which would hand it the user's credentials). Only a symbolic link can lead
/// there (Windows makes no junction to another machine), and making one needs Developer Mode or a privilege; without
/// it the test says so and checks nothing.
#[test]
#[allow(
  clippy::print_stderr,
  reason = "says that nothing was checked where no symbolic link can be made"
)]
fn native_scope_windows_links_to_the_network_are_not_followed() {
  let layout = Layout::new();
  let link = layout.worktree.join("share");
  if let Err(error) = std::os::windows::fs::symlink_dir(r"\\127.0.0.1\orchester-no-share", &link) {
    eprintln!("no symbolic link can be made here ({error}); nothing checked");
    return;
  }
  let scope = layout.scope(RepositoryAccess::Write);
  let outside = orchester_core::native::scope::resolve_local(&text(&link.join("x.txt")));
  for refused in [scope.readable("share/x.txt"), scope.writable("share/x.txt"), outside] {
    assert!(
      matches!(
        refused,
        Err(ScopeError::Unresolved {
          kind: std::io::ErrorKind::PermissionDenied,
          ..
        })
      ),
      "{refused:?}"
    );
  }
}
