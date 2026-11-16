//! The Pipelines page with a pipeline that is not ready (`Pipelines · v2` P8 `vujkk`): *Dependency update* is
//! published, and its step 02 *Review* runs a profile that is no longer available (a pipeline is stored only as
//! published versions, whose steps all run something, D-032, D-034), so it stands under *Needs you*, its card and node
//! are amber and *Ready?* names the fix; three published pipelines whose profiles run on a connection of their own are
//! *Ready*. No agent or command is started.
use orchester_core::{
  id::ResourceId,
  permission::StepPermissions,
  resource::{DeliverStep, Pipeline, PipelineEdit, RepositoryAccess, StepKind},
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
pub(super) fn published(kontur: &mut Kontur, title: &str, steps: &[(&str, ResourceId)]) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let mut before = None;
  for (step, profile) in steps {
    let key = edit.add_step(name(step), before.as_ref());
    assign(&mut edit, &key, *profile);
    before = Some(key);
  }
  create(kontur, title, &edit.validate().expect("steps with profiles are valid"))?;
  Ok(())
}

/// P8's pipeline: *Update* runs on the builder, *Review* on a profile that is no longer available, then *Deliver*.
pub(super) fn dependency_update(kontur: &mut Kontur, builder: ResourceId) -> Result<(), SeedError> {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("empty edit"));
  let update = edit.add_step(name("Update"), None);
  let review = edit.add_step(name("Review"), Some(&update));
  assign(&mut edit, &update, builder);
  assign(&mut edit, &review, ResourceId::generate());
  edit.set_deliver(Some(DeliverStep::default()));
  create(
    kontur,
    "Dependency update",
    &edit.validate().expect("a step whose profile is gone is valid"),
  )?;
  Ok(())
}

fn assign(edit: &mut PipelineEdit, key: &orchester_core::resource::StepKey, profile: ResourceId) {
  edit.update(key, |step| {
    step.kind = StepKind::Agent {
      profile: Some(profile),
      model: None,
      repository: RepositoryAccess::ReadOnly,
      permissions: StepPermissions::default(),
    };
  });
}
