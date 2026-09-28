//! The search path on macOS: the inherited `PATH`, then the usual install folders that exist,
//! then the started program's own folder.

use std::{
  ffi::OsStr,
  fs,
  path::{Path, PathBuf},
};

use orchester_core::command::{Command, CommandRunner};
use orchester_exec::search_path::macos::{extend, home_folders, usual_folders};

use super::RUNNER;

#[test]
fn search_path_appends_missing_folders_after_the_path() {
  let folders = [PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/bin")];
  assert_eq!(
    extend(Some(OsStr::new("/usr/bin:/bin")), &folders),
    OsStr::new("/usr/bin:/bin:/opt/homebrew/bin")
  );
  assert_eq!(extend(None, &folders), OsStr::new("/opt/homebrew/bin:/usr/bin"));
}

#[test]
fn search_path_looks_in_the_home_install_folders_npm_prefix_and_newest_nvm_first() {
  let home = tempfile::tempdir().expect("home");
  let home = home.path();
  fs::write(
    home.join(".npmrc"),
    "registry=https://registry.npmjs.org/\nprefix = ~/.npm-packages\n",
  )
  .expect("npmrc");
  for version in ["v9.11.2", "v22.1.0", "v20.10.0", "system"] {
    fs::create_dir_all(home.join(".nvm/versions/node").join(version).join("bin")).expect("nvm version");
  }

  let folders = home_folders(home);
  let relative: Vec<&Path> = folders
    .iter()
    .map(|folder| folder.strip_prefix(home).expect("in home"))
    .collect();
  assert_eq!(
    relative,
    [
      ".local/bin",
      ".claude/local",
      ".npm-global/bin",
      ".volta/bin",
      ".bun/bin",
      "Library/pnpm",
      ".cargo/bin",
      ".npm-packages/bin",
      ".nvm/versions/node/v22.1.0/bin",
      ".nvm/versions/node/v20.10.0/bin",
      ".nvm/versions/node/v9.11.2/bin",
    ]
    .map(Path::new)
  );
}

#[test]
fn search_path_keeps_only_folders_that_exist() {
  let home = tempfile::tempdir().expect("home");
  let local = home.path().join(".local/bin");
  fs::create_dir_all(&local).expect("~/.local/bin");

  let folders = usual_folders(Some(home.path()));
  let in_home: Vec<&PathBuf> = folders
    .iter()
    .filter(|folder| folder.starts_with(home.path()))
    .collect();
  assert_eq!(in_home, [&local]);
  assert!(folders.iter().all(|folder| folder.is_dir()), "{folders:?}");
}

#[test]
fn search_path_is_what_a_started_program_sees_with_its_own_folder_last() {
  let command = Command::new(Path::new("/usr/bin/env"), Vec::<String>::new())
    .with_environment(vec![("PATH".into(), "/bin".into())]);
  let output = RUNNER.run(&command).expect("env runs");
  let path = output
    .stdout
    .lines()
    .find_map(|line| line.strip_prefix("PATH="))
    .expect("PATH is set");

  let entries: Vec<&str> = path.split(':').collect();
  assert_eq!(entries.first(), Some(&"/bin"), "{path}");
  assert_eq!(
    entries.last(),
    Some(&"/usr/bin"),
    "the program's folder comes last: {path}"
  );
  for folder in usual_folders(Some(Path::new("/nonexistent-home"))) {
    assert!(
      entries.contains(&folder.to_str().expect("utf-8")),
      "{folder:?} in {path}"
    );
  }
}
