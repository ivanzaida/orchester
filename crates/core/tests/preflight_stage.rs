//! A merge preflight's causes (ORC-644): Git's last words bounded and without a URL's user part, paths relative to
//! the project. `cargo test -p orchester-core --test preflight_stage preflight_stage`.

use std::{io, path::Path};

use orchester_core::integration::{GitCause, IoCause, PreflightCause, PreflightStage};

#[test]
fn preflight_stage_git_cause_keeps_the_last_lines_bounded_and_redacted() {
  let mut stderr = String::new();
  for line in 1..=10 {
    stderr.push_str(&format!("line {line}\n\n"));
  }
  stderr.push_str("fatal: could not read https://user:hunter2@example.invalid/repo.git\n");
  let cause = GitCause::new(Some(128), stderr.as_bytes());
  assert_eq!(cause.exit(), Some(128));
  assert_eq!(cause.tail().lines().count(), 4);
  assert!(cause.tail().starts_with("line 8\n"), "{}", cause.tail());
  assert!(!cause.tail().contains("hunter2"), "{}", cause.tail());
  let long = "x".repeat(2000);
  assert_eq!(GitCause::new(Some(1), long.as_bytes()).tail().chars().count(), 600);
  assert_eq!(
    PreflightCause::Git(cause).to_string().lines().next(),
    Some("git exited with 128: line 8")
  );
}

#[test]
fn preflight_stage_io_cause_names_its_path_inside_the_project_only() {
  let root = Path::new("/projects/app");
  let error = io::Error::from_raw_os_error(13);
  let inside = IoCause::new(&error).at(&root.join(".orchester/orchester.db-wal"));
  let PreflightCause::Io { path, kind, code, text } = PreflightCause::io(&inside, root) else {
    panic!("an OS error");
  };
  assert_eq!(path.as_deref(), Some(".orchester/orchester.db-wal"));
  assert_eq!(kind, io::ErrorKind::PermissionDenied);
  assert_eq!(code, Some(13));
  assert!(!text.contains("os error"), "{text}");
  let outside = IoCause::new(&error).at(Path::new("/elsewhere/file"));
  assert!(matches!(
    PreflightCause::io(&outside, root),
    PreflightCause::Io { path: None, .. }
  ));
  assert_eq!(PreflightStage::IntegrationLock.name(), "integration_lock");
}
