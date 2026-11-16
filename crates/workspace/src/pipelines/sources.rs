//! A pipeline draft's sources (Pipeline settings › Sources `Q10UQ`, the step inspector's Sources `O6piA3`): the
//! pipeline's own, which every step's agent reads, and each step's. They change the draft only: runs get them once a
//! version with them is published, which asks a person when it lets agents read more
//! ([`super::widening::publish_reasons`]).

use std::collections::BTreeMap;

use orchester_core::{
  resource::{PipelineEdit, PipelineSources, StepKey},
  source::SourcePath,
};

use crate::sources::check::Place;

/// Which list of the draft a source is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listing {
  Pipeline,
  Step(StepKey),
}

/// The draft's sources with `change` made to the list `listing` names.
fn changed(edit: &PipelineEdit, listing: &Listing, change: impl FnOnce(&mut Vec<SourcePath>)) -> PipelineSources {
  let sources = edit.sources();
  let mut pipeline = sources.pipeline().to_vec();
  let mut steps: BTreeMap<StepKey, Vec<SourcePath>> = edit
    .steps()
    .iter()
    .map(|step| (step.key.clone(), sources.step(&step.key).to_vec()))
    .collect();
  match listing {
    Listing::Pipeline => change(&mut pipeline),
    Listing::Step(key) => change(steps.entry(key.clone()).or_default()),
  }
  PipelineSources::new(pipeline, steps)
}

/// Adds `path` to the list `listing` names.
pub fn add(edit: &mut PipelineEdit, listing: &Listing, path: SourcePath) {
  let sources = changed(edit, listing, |paths| paths.push(path));
  edit.set_sources(sources);
}

/// Takes `path` off the list `listing` names.
pub fn remove(edit: &mut PipelineEdit, listing: &Listing, path: &SourcePath) {
  let sources = changed(edit, listing, |paths| paths.retain(|source| source != path));
  edit.set_sources(sources);
}

/// Where a path for `listing` is checked: outside the project, its sources and the draft's sources a step there reads.
pub fn place(
  repo_root: &std::path::Path,
  project: &str,
  project_sources: &[SourcePath],
  edit: &PipelineEdit,
  listing: &Listing,
) -> Place {
  let sources = edit.sources();
  let mut listed: Vec<SourcePath> = project_sources.iter().chain(sources.pipeline()).cloned().collect();
  if let Listing::Step(key) = listing {
    listed.extend(sources.step(key).iter().cloned());
  }
  Place::new(repo_root, project, listed)
}

/// The steps that list sources of their own, each with how many, in the draft's order
/// (`03 Implement adds 1 more on its step`).
pub fn step_counts(edit: &PipelineEdit) -> Vec<(String, usize)> {
  edit
    .steps()
    .iter()
    .map(|step| (step.name.as_str().to_owned(), edit.sources().step(&step.key).len()))
    .filter(|(_, count)| *count > 0)
    .collect()
}
