//! Host-side free-space preflight for a project that sets an absolute `CARGO_TARGET_DIR`.
//! Relative targets may resolve differently in each worktree and are not checked here.

use std::{fs, io, path::Path};

use orchester_core::{
  run::{BuildTargetState, PreflightProblem, build_space_problem},
  store::SettingsStore,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use super::failed;

const CARGO_TARGET_DIR: &str = "CARGO_TARGET_DIR";

pub(super) fn preflight(db: &ProjectDb, root: &Path) -> Result<(), Rejection> {
  let environment = db
    .project_environment()
    .map_err(|error| failed("cannot read the project's environment", &error))?;
  let Some(configured) = environment.get(CARGO_TARGET_DIR) else {
    return Ok(());
  };
  let target = Path::new(configured);
  if !target.is_absolute() {
    return Ok(());
  }
  let state = target_state(target).map_err(|error| unavailable(target, &error))?;
  let ancestor = existing_ancestor(target).map_err(|error| unavailable(target, &error))?;
  let resolved = fs::canonicalize(ancestor).map_err(|error| unavailable(target, &error))?;
  let available = available_space(target, &resolved).map_err(|error| unavailable(target, &error))?;
  if let Some(problem) = build_space_problem(target.to_path_buf(), available, state) {
    return Err(Rejection::Preflight {
      problems: vec![problem],
    });
  }
  tracing::debug!(project = %root.display(), target = %target.display(), available, "shared Cargo target has space");
  Ok(())
}

fn available_space(target: &Path, resolved: &Path) -> io::Result<u64> {
  // The test-agent runner can exercise the real preflight and UI on hosts without a small
  // volume. The override applies only to the exact disposable target named by the test.
  #[cfg(feature = "test-agent")]
  if let Some(expected) = std::env::var_os("ORCHESTER_TEST_SPACE_TARGET")
    && Path::new(&expected) == target
  {
    let bytes = std::env::var("ORCHESTER_TEST_SPACE_BYTES")
      .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    return bytes
      .parse()
      .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error));
  }
  #[cfg(not(feature = "test-agent"))]
  let _ = target;
  fs4::available_space(resolved)
}

fn target_state(target: &Path) -> io::Result<BuildTargetState> {
  if !target.try_exists()? {
    return Ok(BuildTargetState::Empty);
  }
  for entry in fs::read_dir(target)? {
    let entry = entry?;
    if !entry.file_type()?.is_dir() {
      continue;
    }
    if has_compiled_deps(&entry.path())? {
      return Ok(BuildTargetState::Populated);
    }
    for profile in fs::read_dir(entry.path())? {
      let profile = profile?;
      if profile.file_type()?.is_dir() && has_compiled_deps(&profile.path())? {
        return Ok(BuildTargetState::Populated);
      }
    }
  }
  Ok(BuildTargetState::Empty)
}

fn has_compiled_deps(profile: &Path) -> io::Result<bool> {
  let deps = profile.join("deps");
  Ok(deps.try_exists()? && fs::read_dir(deps)?.next().transpose()?.is_some())
}

fn existing_ancestor(target: &Path) -> io::Result<&Path> {
  for candidate in target.ancestors() {
    // Inspect the path itself before following it. A dangling link is present but its target
    // volume cannot be measured; walking past it would measure the wrong (parent) volume.
    match fs::symlink_metadata(candidate) {
      Ok(_) => match fs::metadata(candidate) {
        Ok(metadata) if metadata.is_dir() => return Ok(candidate),
        Ok(_) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
        Err(error) => return Err(error),
      },
      Err(error) if error.kind() == io::ErrorKind::NotFound => {}
      Err(error) => return Err(error),
    }
  }
  Err(io::Error::new(
    io::ErrorKind::NotFound,
    "the Cargo target's volume is missing",
  ))
}

fn unavailable(target: &Path, error: &io::Error) -> Rejection {
  tracing::warn!(target = %target.display(), %error, "cannot measure shared Cargo target space");
  Rejection::Preflight {
    problems: vec![PreflightProblem::BuildSpaceUnavailable {
      target: target.to_path_buf(),
    }],
  }
}
