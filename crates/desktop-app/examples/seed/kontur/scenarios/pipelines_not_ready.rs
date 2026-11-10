//! The Pipelines page with a pipeline that is not ready (`Pipelines · v2` P8 `vujkk`): *Dependency update* is a draft
//! whose step 02 *Review* has no agent profile, so it stands under *Needs you*, its card and node are amber and
//! *Ready?* names the fix; three published pipelines whose profiles run on a connection of their own are *Ready*. No
//! agent or command is started.
use orchester_core::{
  id::ResourceId,
  permission::StepPermissions,
  resource::{DeliverStep, Pipeline, PipelineEdit, RepositoryAccess, StepKind},
  store::ResourceStore,
};

use super::pipelines::{create, profile};
use crate::kontur::{Kontur, SeedError, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let builder = profile(kontur, "Builder", &["cargo test"], true)?;
  let reviewer = profile(kontur, "Reviewer", &["cargo check"], true)?;
  published(
    kontur,
    "Feature delivery",
    &[("Plan", builder), ("Implement", builder), ("Review", reviewer)],
  )?;
  published(kontur, "Quick fix", &[("Fix", builder), ("Check", reviewer)])?;
  published(kontur, "Release notes", &[("Write", builder)])?;
  dependency_update(kontur, builder)
}

/// A published pipeline whose steps run in the order given, each on its profile.
fn published(kontur: &mut Kontur, title: &str, steps: &[(&str, ResourceId)]) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let mut before = None;
  for (step, profile) in steps {
    let key = edit.add_step(name(step), before.as_ref());
    assign(&mut edit, &key, Some(*profile));
    before = Some(key);
  }
  let id = create(kontur, title, &edit.validate().expect("steps with profiles are valid"))?;
  kontur.db.publish_draft::<Pipeline>(id, kontur.now)?;
  Ok(())
}

/// The draft of P8: *Update* runs on the builder, *Review* has no profile, then *Deliver*.
fn dependency_update(kontur: &mut Kontur, builder: ResourceId) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty draft"));
  let update = edit.add_step(name("Update"), None);
  let review = edit.add_step(name("Review"), Some(&update));
  assign(&mut edit, &update, Some(builder));
  assign(&mut edit, &review, None);
  edit.set_deliver(Some(DeliverStep::default()));
  create(
    kontur,
    "Dependency update",
    &edit.validate().expect("a step with no profile is valid in a draft"),
  )?;
  Ok(())
}

fn assign(edit: &mut PipelineEdit, key: &orchester_core::resource::StepKey, profile: Option<ResourceId>) {
  edit.update(key, |step| {
    step.kind = StepKind::Agent {
      profile,
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions::default(),
    };
  });
}
