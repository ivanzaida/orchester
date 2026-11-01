//! The reviewer of the test projects: none unless a test chooses the default one.

use orchester_core::{
  id::ConnectionId,
  resource::ModelId,
  run::review::{AutoModeSettings, DEFAULT_REVIEW_MODEL, ReviewerChoice},
};
use orchester_store::ProjectDb;

use super::TestProject;

/// Runs are in Manual, so a request waits for the person only once the reviewer leaves it to them. These projects
/// have no reviewer (the chosen connection is gone), so their requests ask the person at once; a test of the
/// reviewer chooses one ([`with_default_reviewer`]).
pub(super) fn none(db: &mut ProjectDb) {
  db.save_auto_mode_settings(&AutoModeSettings {
    reviewer: Some(ReviewerChoice {
      connection: (ConnectionId::generate()).into(),
      model: ModelId::new(DEFAULT_REVIEW_MODEL).expect("a valid model"),
      effort: None,
      threshold: None,
    }),
  })
  .expect("no reviewer");
}

/// `project` with the default reviewer: its first Claude Code connection, else its first connection a reviewer runs
/// on.
pub fn with_default_reviewer(project: TestProject) -> TestProject {
  ProjectDb::open(project.repo.path())
    .expect("open")
    .save_auto_mode_settings(&AutoModeSettings::default())
    .expect("the default reviewer");
  project
}
