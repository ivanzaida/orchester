//! The `pipelines-unordered` scenario, which reaches `Pipelines · v2` P16c `YHAyx` in a QA copy: it leaves the marker
//! that has Feature delivery's Review wait for Create pull request in the panel's edit, which the QA app arms.
//! `cargo test -p orchester-app --test seed pipelines_unordered`.
use std::fs;

use orchester_app::test_support::pipeline_edit::{MARKER, arm_from_marker};
use orchester_core::{resource::ResourceKind, store::ResourceStore};

use super::Seed;

#[test]
fn seed_pipelines_unordered_leaves_the_marker_that_makes_review_and_create_pull_request_wait_for_each_other() {
  let seed = Seed::of("pipelines-unordered");
  let feature = seed
    .db()
    .resources(ResourceKind::Pipeline)
    .unwrap()
    .into_iter()
    .find(|row| row.name().as_str() == "Feature delivery")
    .unwrap()
    .id();
  let database = &seed.seeded.app_database;
  let marker = fs::read_to_string(database.with_file_name(MARKER)).unwrap();
  assert_eq!(marker, format!("{feature}\nreview create-pull-request\n"));
  assert_eq!(arm_from_marker(database), Ok(true), "the QA app arms it");
}
