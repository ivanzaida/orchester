//! The search path on Windows: the inherited `PATH`, then the `Path` Windows keeps for the machine
//! and the user, so the app started by another program (its installer) still finds git. Every test
//! adds that `Path` first, as the app does as it starts (`search_path::read_login_shell`); the
//! tests of the `process` binary run without it.
#![cfg(windows)]

use std::{
  env,
  ffi::OsStr,
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_core::command::{Command, CommandRunner};
use orchester_exec::{
  ProcessRunner,
  search_path::{self, FolderSource, SearchPath, windows::registry_folders},
};

const RUNNER: ProcessRunner = ProcessRunner::new(Duration::from_secs(30));

/// The system folder alone, as the `PATH` of a program started without Git's folder.
fn system_folder() -> PathBuf {
  PathBuf::from(env::var_os("SystemRoot").expect("Windows names its folder")).join("System32")
}

/// Whether `one` and `other` name the same folder, whatever the case of their letters or a separator at the end.
fn same_folder(one: &Path, other: &Path) -> bool {
  let plain = |folder: &Path| folder.to_string_lossy().trim_end_matches('\\').to_ascii_lowercase();
  plain(one) == plain(other)
}

#[test]
fn registry_path_follows_the_inherited_path_without_its_folders_again() {
  let inherited = OsStr::new(r"C:\Windows\System32;C:\Tools\bin\");
  let registry = [r"c:\windows\system32", r"C:\tools\BIN", r"C:\Program Files\Git\cmd"].map(PathBuf::from);

  let search = SearchPath::merge(Some(inherited), &[], &[]).with_registry(&registry);
  let folders: Vec<(&Path, FolderSource)> = search
    .folders()
    .iter()
    .map(|folder| (folder.path(), folder.source()))
    .collect();
  assert_eq!(
    folders,
    [
      (Path::new(r"C:\Windows\System32"), FolderSource::Inherited),
      (Path::new(r"C:\Tools\bin\"), FolderSource::Inherited),
      (Path::new(r"C:\Program Files\Git\cmd"), FolderSource::Registry),
    ]
  );
}

#[test]
fn registry_path_holds_the_machines_folders_expanded() {
  let folders = registry_folders();
  let system = system_folder();
  assert!(
    folders.iter().any(|folder| same_folder(folder, &system)),
    "the machine's Path names %SystemRoot%\\System32: {folders:?}"
  );
  assert!(
    folders.iter().all(|folder| !folder.to_string_lossy().contains('%')),
    "{folders:?}"
  );
}

#[test]
fn registry_path_is_searched_after_this_process_path() {
  search_path::read_login_shell();
  let search = SearchPath::current();
  for folder in registry_folders() {
    assert!(
      search.folders().iter().any(|known| same_folder(known.path(), &folder)),
      "{} is searched: {:?}",
      folder.display(),
      search.folders()
    );
  }
}

#[test]
fn registry_path_finds_git_for_a_program_whose_path_lacks_it() {
  search_path::read_login_shell();
  let system = system_folder();
  let output = RUNNER
    .run(&Command::new(Path::new("where"), ["git"]).with_env("PATH", &system.to_string_lossy()))
    .expect("where runs");
  assert!(output.success(), "{}", output.stderr);
  let found = output.stdout.lines().next().map(Path::new).expect("where names git");
  assert!(found.ends_with("git.exe"), "{}", output.stdout);
  let folder = found.parent().expect("git's folder");
  assert!(
    registry_folders().iter().any(|known| same_folder(known, folder)),
    "{}",
    folder.display()
  );
}

#[test]
fn registry_path_is_the_path_of_a_program_with_an_explicit_environment() {
  search_path::read_login_shell();
  let variables: Vec<(String, String)> = env::vars()
    .filter(|(name, _)| name.eq_ignore_ascii_case("SYSTEMROOT"))
    .collect();
  let output = RUNNER
    .run(&Command::new(Path::new("cmd"), ["/C", "set"]).with_environment(variables))
    .expect("the program runs");
  let search = SearchPath::merge(None, &[], &[]).with_registry(&registry_folders());
  let expected = format!("PATH={}", search.joined().to_string_lossy());
  assert!(output.stdout.lines().any(|line| line == expected), "{}", output.stdout);
}

/// Started with a `PATH` without Git's folder (as the installer started the app), git still runs.
#[test]
fn registry_path_runs_git_whatever_this_process_path() {
  search_path::read_login_shell();
  let output = RUNNER
    .run(&Command::new(Path::new("git"), ["--version"]))
    .expect("git runs");
  assert!(output.stdout.starts_with("git version"), "{}", output.stdout);
}
