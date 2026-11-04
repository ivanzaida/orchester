//! The usage area of the local link: what a run's steps used, counted from the invocations and reviews the project
//! database has recorded, both read once per ask.

use std::ops::Deref;

use orchester_core::{run::execution_tokens, store::ActivityStore};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  usage::{ExecutionUsage, StepUsage, StepUsageRequest, UsageLink},
};

impl<D: Deref<Target = ProjectDb>> UsageLink for LocalLink<D> {
  fn step_usage(&self, request: &StepUsageRequest) -> Result<StepUsage, LinkError> {
    let invocations = self.0.invocations(request.run).map_err(|error| read_failed(&error))?;
    let reviews = self
      .0
      .reviewer_usage(request.run)
      .map_err(|error| read_failed(&error))?;
    let used = request
      .executions
      .iter()
      .map(|&execution| ExecutionUsage {
        execution,
        tokens: execution_tokens(&invocations, &reviews, execution),
      })
      .collect();
    Ok(StepUsage { used })
  }
}
