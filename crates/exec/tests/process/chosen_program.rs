//! A program file a person chose as a CLI connection's program: examined, never run.

use std::{fs, path::Path};

use orchester_exec::search_path::{ChosenProgramError, chosen_program};

#[test]
fn chosen_program_file_is_accepted_as_chosen() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let program = folder.path().join("claude");
  fs::write(&program, "").expect("a program file");
  #[cfg(unix)]
  set_mode(&program, 0o755);

  assert_eq!(chosen_program(&program).expect("a program file"), program);
}

#[test]
fn chosen_program_that_is_missing_relative_or_a_folder_is_refused() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let file = folder.path().join("claude");
  fs::write(&file, "").expect("a file");

  let missing = folder.path().join("missing");
  assert!(matches!(chosen_program(&missing), Err(ChosenProgramError::Missing(path)) if path == missing));
  let under_a_file = file.join("claude");
  assert!(matches!(
    chosen_program(&under_a_file),
    Err(ChosenProgramError::Missing(_))
  ));
  assert!(matches!(
    chosen_program(Path::new("bin/claude")),
    Err(ChosenProgramError::NotAbsolute(_))
  ));
  assert!(matches!(
    chosen_program(folder.path()),
    Err(ChosenProgramError::NotAFile(path)) if path == folder.path()
  ));
}

#[cfg(unix)]
#[test]
fn chosen_program_without_an_execute_bit_is_refused() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let program = folder.path().join("claude");
  fs::write(&program, "").expect("a file");
  set_mode(&program, 0o644);

  assert!(matches!(
    chosen_program(&program),
    Err(ChosenProgramError::NotExecutable(path)) if path == program
  ));
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
  use std::os::unix::fs::PermissionsExt;

  fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("permissions");
}
