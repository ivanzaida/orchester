//! Links that lead out of the worktree: a symbolic link on macOS, a junction on Windows (made with
//! `mklink /J`, which needs no privilege). Reads and writes through them are refused, and listing does
//! not follow them.

use std::path::Path;

use orchester_agent::FileTools;
use orchester_core::permission::RepositoryAccess;

use crate::{Layout, failed, list_input, ok, read_input, ripgrep, search_input, write_input};

#[cfg(unix)]
fn link_folder(link: &Path, target: &Path) {
  std::os::unix::fs::symlink(target, link).expect("a symbolic link");
}

#[cfg(windows)]
fn link_folder(link: &Path, target: &Path) {
  // `cmd` reads a `/` in an argument as the start of a switch.
  let native = |path: &Path| path.display().to_string().replace('/', "\\");
  let status = std::process::Command::new("cmd")
    .args(["/C", "mklink", "/J"])
    .arg(native(link))
    .arg(native(target))
    .output()
    .expect("mklink runs");
  assert!(
    status.status.success(),
    "mklink: {}",
    String::from_utf8_lossy(&status.stderr)
  );
}

#[test]
fn files_links_out_of_the_worktree_are_refused() {
  let layout = Layout::temporary();
  link_folder(&layout.worktree.join("escape"), &layout.outside);
  let tools = layout.tools();
  let read = failed(tools.read_file(&read_input("escape/secret.txt", None, None)));
  assert!(read.contains("outside the folders this step may read"), "{read}");
  let write = failed(tools.write_file(&write_input("escape/planted.txt", "x")));
  assert!(write.contains("outside the folders this step may write"), "{write}");
  assert!(!layout.outside.join("planted.txt").exists());
  let listed = ok(tools.list_files(&list_input(None, None)));
  assert!(!listed.contains("secret.txt"), "{listed}");
  let ripgrep = FileTools::new(layout.scope(RepositoryAccess::Write), Some(ripgrep()));
  let listed = ok(ripgrep.list_files(&list_input(None, None)));
  assert!(!listed.contains("secret.txt"), "{listed}");
  let found = ok(ripgrep.search(&search_input("secret", None, None, None)));
  assert!(found.starts_with("no matches"), "{found}");
}
