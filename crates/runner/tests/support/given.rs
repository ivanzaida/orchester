//! The project's Builder giving its agents instructions, skills and tools (`docs/design/team-configuration.md`,
//! "Decided 2026-11-09"): nothing else gives a local run's agent one. Published as the Builder's next version, which a
//! run started afterwards pins.

use std::path::Path;

use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, Draft, ResourceKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

/// Adds each of `resources`, an instruction, skill or tool of the project, to what the Builder gives.
pub fn give_builder(root: &Path, resources: &[ResourceId]) {
  let mut db = ProjectDb::open(root).expect("open");
  let builder = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .into_iter()
    .find(|resource| resource.slug().as_str() == "builder")
    .expect("the Builder")
    .id();
  let latest = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("published");
  let body = latest.body();
  let (mut instructions, mut skills, mut tools) = (
    body.instruction_resources().clone(),
    body.skills().clone(),
    body.tools().clone(),
  );
  for id in resources {
    match db.resource(*id).expect("read").expect("a resource").kind() {
      ResourceKind::Instruction => instructions = instructions.with(*id),
      ResourceKind::Skill => skills = skills.with(*id),
      ResourceKind::Tool => tools = tools.with(*id),
      other => panic!("a profile gives no {other:?}"),
    }
  }
  let given = body.clone().with_given(instructions, skills, tools);
  db.save_draft(builder, &Draft::new(given, Some(latest.number()), Timestamp::now()))
    .expect("draft");
  db.publish_draft::<AgentProfile>(builder, Timestamp::now())
    .expect("publish");
}
