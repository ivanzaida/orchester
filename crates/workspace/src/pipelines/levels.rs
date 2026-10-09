//! Steps grouped by dependency level, as the pipeline library lists a pipeline's stages and the
//! canvas places its rows.

use std::collections::BTreeMap;

use orchester_core::resource::{Step, StepKey};

/// Step positions grouped by dependency level, each level in list order. Dependencies on
/// unknown steps are ignored, and so is `closing` — the dependency `(waiting, dependency)`
/// that closes a cycle — so a pipeline being edited into a broken state still has a layout.
pub fn levels_ignoring(steps: &[Step], closing: Option<(usize, usize)>) -> Vec<Vec<usize>> {
  let dependencies = dependencies(steps, closing);
  let mut level: Vec<Option<usize>> = vec![None; steps.len()];
  let mut visiting = vec![false; steps.len()];
  for position in 0..steps.len() {
    depth(position, &dependencies, &mut level, &mut visiting);
  }
  let mut grouped: Vec<Vec<usize>> = Vec::new();
  for (position, found) in level.into_iter().enumerate() {
    let found = found.unwrap_or(0);
    if grouped.len() <= found {
      grouped.resize(found + 1, Vec::new());
    }
    grouped[found].push(position);
  }
  grouped.retain(|group| !group.is_empty());
  grouped
}

/// Positions of each step's dependencies, without unknown steps and the `closing` one.
pub fn dependencies(steps: &[Step], closing: Option<(usize, usize)>) -> Vec<Vec<usize>> {
  let index: BTreeMap<&StepKey, usize> = steps.iter().enumerate().map(|(i, step)| (&step.key, i)).collect();
  steps
    .iter()
    .enumerate()
    .map(|(position, step)| {
      step
        .depends_on
        .iter()
        .filter_map(|key| index.get(key).copied())
        .filter(|&dependency| closing != Some((position, dependency)))
        .collect()
    })
    .collect()
}

/// [`levels_ignoring`] for a valid pipeline.
pub fn levels(steps: &[Step]) -> Vec<Vec<usize>> {
  levels_ignoring(steps, None)
}

/// Longest chain of dependencies above `position`; an edge back into the chain being walked
/// (a cycle other than the ignored one) counts as no dependency.
fn depth(position: usize, dependencies: &[Vec<usize>], level: &mut [Option<usize>], visiting: &mut [bool]) -> usize {
  if let Some(found) = level[position] {
    return found;
  }
  if visiting[position] {
    return 0;
  }
  visiting[position] = true;
  let mut found = 0;
  for &dependency in &dependencies[position] {
    if !visiting[dependency] {
      found = found.max(depth(dependency, dependencies, level, visiting) + 1);
    }
  }
  visiting[position] = false;
  level[position] = Some(found);
  found
}
