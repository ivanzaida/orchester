//! The `pipelines-new` scenario (`Pipelines · v2` P14–P14c, PV2-22): P8's four pipelines, and of what the starter looks
//! for only *Explorer*, *Builder*, *Run test suite* and *Create pull request*, so a new pipeline's starter has *Plan*
//! and *Review* with no profile.

use orchester_core::{resource::ResourceKind, store::ResourceStore};
use orchester_workspace::pipelines::data::Assignables;

use super::Seed;

#[test]
fn seed_pipelines_new_leaves_the_starters_plan_and_review_without_a_profile() {
  let seed = Seed::of("pipelines-new");
  let db = seed.db();
  let names: Vec<String> = db
    .resources(ResourceKind::Pipeline)
    .expect("the pipelines read")
    .iter()
    .map(|resource| resource.name().as_str().to_owned())
    .collect();
  for name in ["Dependency update", "Feature delivery", "Quick fix", "Release notes"] {
    assert!(names.iter().any(|listed| listed == name), "{name} in {names:?}");
  }
  let assignables = Assignables::load(&db).expect("what steps can run reads");
  for found in ["Explorer", "Builder"] {
    assert!(assignables.profile_named(found).is_some(), "{found} is published");
  }
  for missing in ["Planner", "Reviewer"] {
    assert!(assignables.profile_named(missing).is_none(), "no {missing}");
  }
  for tool in ["Run test suite", "Create pull request"] {
    assert!(assignables.tool_named(tool).is_some(), "{tool} is published");
  }
}
