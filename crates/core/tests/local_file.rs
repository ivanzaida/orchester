//! A path that names another machine or a device is refused by its text, before anything reads it; a local file
//! resolves as the file system resolves it, written without `\\?\`. Run with
//! `cargo test -p orchester-core --test local_file local`.

use orchester_core::permission::{resolve_local, stays_local};

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
