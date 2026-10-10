//! A path that names another machine or a device is refused by its text, before anything reads it; a local file
//! resolves as the file system resolves it, written without `\\?\`. Run with
//! `cargo test -p orchester-core --test local_file local`.

use orchester_core::permission::{ProgramPlace, program_place, resolve_local, stays_local};

#[test]
fn local_files_resolve_as_written() {
  let file = std::env::current_exe().expect("the test program");
  assert!(stays_local(&file).is_ok());
  let resolved = resolve_local(&file).expect("a local file resolves");
  assert!(
    !resolved.to_string_lossy().starts_with(r"\\?\"),
    "{}",
    resolved.display()
  );
  assert!(resolved.is_file());
}

#[cfg(windows)]
#[test]
fn local_network_and_device_paths_are_refused_by_their_text() {
  for path in [
    r"\\server\share\mcp.exe",
    r"\\?\UNC\server\share\mcp.exe",
    r"\\.\pipe\mcp",
    "//server/share/mcp.exe",
  ] {
    let path = std::path::Path::new(path);
    assert!(stays_local(path).is_err(), "{}", path.display());
    assert!(resolve_local(path).is_err(), "{}", path.display());
  }
}

/// A stored program must lie, and lead, outside the project's repository; the file found is what is checked, not a
/// copy of what it leads to.
#[test]
fn local_programs_inside_the_repository_are_refused() {
  let exe = std::env::current_exe().expect("the test program");
  let folder = exe.parent().expect("the test program's folder");
  let repository = folder.join("local-file-repository");
  std::fs::create_dir_all(&repository).expect("a repository folder");
  assert_eq!(program_place(&exe, &repository), Ok(()));
  let planted = repository.join("planted.exe");
  std::fs::write(&planted, b"agent's").expect("a planted program");
  assert!(matches!(
    program_place(&planted, &repository),
    Err(ProgramPlace::InRepository(_))
  ));
  assert_eq!(
    program_place(&repository.join("missing.exe"), &repository),
    Err(ProgramPlace::NotLocal)
  );
}
