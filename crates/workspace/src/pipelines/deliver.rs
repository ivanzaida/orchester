//! The Deliver step as the pipeline editor sets it (ORC-19, `docs/design/orchester-workspace.md` › *Delivery*): which
//! agent profiles its Integrate step may resolve conflicts with, and which step it then works in. The merge queue
//! sends conflicts to the same step (`crate::merge_queue`).

use orchester_core::{
  id::ResourceId,
  resource::{DeliverStep, Pipeline, RepositoryAccess, Step, StepKey, StepKind},
};

/// The integrator profile the editor's *Resolve conflicts with* holds: one the Deliver step names, or `None` for the
/// profile of the step that implements the change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntegratorChoice(pub Option<ResourceId>);

/// The agent profiles the Integrate step may work with in `pipeline`: those of its steps that may write in the
/// worktree, in pipeline order, each once.
pub fn integrator_profiles(pipeline: &Pipeline) -> Vec<ResourceId> {
  let mut profiles = Vec::new();
  for step in writing(pipeline.steps()) {
    if let Some(profile) = step.kind.executor()
      && !profiles.contains(&profile)
    {
      profiles.push(profile);
    }
  }
  profiles
}

/// The step of `pipeline` the Integrate step works in, among the steps `considered` admits (a run's: those it ran):
/// the last one that may write and works with `deliver`'s integrator profile; without one chosen, the last writing
/// step that merges the base when it moves (`Pipeline::takes_base_moves`), else the last writing step. `None` when
/// none fits: conflicts then go to a person.
pub fn integrating_step<'a>(
  pipeline: &'a Pipeline,
  deliver: &DeliverStep,
  considered: impl Fn(&StepKey) -> bool,
) -> Option<&'a Step> {
  let writing: Vec<&Step> = writing(pipeline.steps())
    .filter(|step| considered(&step.key))
    .collect();
  match deliver.integrator() {
    Some(profile) => writing
      .iter()
      .rev()
      .find(|step| step.kind.executor() == Some(profile))
      .copied(),
    None => writing
      .iter()
      .rev()
      .find(|step| pipeline.takes_base_moves(&step.key))
      .or_else(|| writing.last())
      .copied(),
  }
}

/// The agent steps of `steps` whose agent may write in the worktree.
fn writing(steps: &[Step]) -> impl Iterator<Item = &Step> {
  steps.iter().filter(|step| {
    matches!(
      step.kind,
      StepKind::Agent {
        repository: RepositoryAccess::Write,
        ..
      }
    )
  })
}
