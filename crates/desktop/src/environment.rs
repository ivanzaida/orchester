//! Project settings › Environment as data: the variables the project adds to the environment of
//! every agent CLI and command tool its runs start, and whether to suggest a shared Cargo build
//! folder. Blocks on SQLite and the file system; the UI calls it off the UI thread.

use std::path::{Path, PathBuf};

use orchester_core::{resource::CommandEnvironment, store::SettingsStore};
use orchester_store::{ProjectDb, StoreError};

/// The variable that moves Cargo's build output out of each worktree.
pub const CARGO_TARGET_DIR: &str = "CARGO_TARGET_DIR";

/// The project's environment additions and what the settings suggest for its repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectEnvironment {
  pub variables: CommandEnvironment,
  /// For a Rust repository (a `Cargo.toml` at its root) that does not set [`CARGO_TARGET_DIR`]:
  /// one build folder beside the worktrees, `<repo>/.orchester/target`, shared by every run
  /// instead of one per worktree. Only suggested; nothing is set until the person adds it.
  pub suggested_target: Option<PathBuf>,
}

impl lurq::app::component::DevtoolsInspectable for ProjectEnvironment {}

impl ProjectEnvironment {
  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    let variables = ProjectDb::open(repo_root)?.project_environment()?;
    let rust = repo_root.join("Cargo.toml").is_file();
    let suggested_target =
      (rust && variables.get(CARGO_TARGET_DIR).is_none()).then(|| repo_root.join(".orchester").join("target"));
    Ok(Self {
      variables,
      suggested_target,
    })
  }
}

pub fn save_in(repo_root: &Path, variables: &CommandEnvironment) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_project_environment(variables)
}
