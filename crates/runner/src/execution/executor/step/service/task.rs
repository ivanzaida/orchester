//! The service tools a run's invocation gets, and where they record a crash and a start that waits for service slots:
//! in the invocation's transcript, as runner notes the run's conversation shows. And the service slots a scheduler's
//! runs take.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::{InvocationId, StepExecutionId},
  resource::CommandEnvironment,
  service::{ServiceName, SlotStep, StackName, UiDriver},
};
use parking_lot::Mutex;

use super::{
  Holding, ServiceSlots, Services, StepServices,
  launch::{published, published_stacks},
};
use crate::execution::{
  Scheduler,
  executor::{
    run::RunTask,
    step::{runner_note, start::problem},
  },
};

impl Scheduler {
  /// This scheduler's services taking `slots`, before it is shared: what tests give it instead of the computer's.
  #[must_use]
  pub fn with_service_slots(mut self, slots: ServiceSlots) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      inner.services = Arc::new(Services::new(slots));
    }
    self
  }
}

/// The project's published services as the step's tools are offered them.
pub(in crate::execution::executor::step) struct ServiceNames {
  /// Their names, which the step's service tools take.
  pub(in crate::execution::executor::step) names: Vec<ServiceName>,
  /// Whether one of them has the `native` or `browser` UI driver, which the `ui_*` tools drive.
  pub(in crate::execution::executor::step) driven_ui: bool,
}

impl RunTask {
  /// The project's published services, by name.
  pub(in crate::execution::executor::step) fn service_names(&self) -> Result<ServiceNames, RunnerProblem> {
    let services =
      published(&self.root).map_err(|error| problem(format!("cannot read the project's services: {error}")))?;
    Ok(ServiceNames {
      names: services.iter().map(|service| service.name().clone()).collect(),
      driven_ui: services
        .iter()
        .any(|service| matches!(service.ui_driver(), UiDriver::Native | UiDriver::Browser)),
    })
  }

  /// The names of the project's published stacks, which `start_stack` takes.
  pub(in crate::execution::executor::step) fn stack_names(&self) -> Result<Vec<StackName>, RunnerProblem> {
    let stacks =
      published_stacks(&self.root).map_err(|error| problem(format!("cannot read the project's stacks: {error}")))?;
    Ok(stacks.iter().map(|stack| stack.name().clone()).collect())
  }

  /// The service tools of `invocation`, whose agent works on `execution` in `worktree` with the project's
  /// `environment`; the Services tab reaches the invocation's services through them until it ends.
  pub(in crate::execution::executor::step) fn step_services(
    &self,
    (execution, invocation): (StepExecutionId, InvocationId),
    worktree: &Path,
    environment: &CommandEnvironment,
  ) -> StepServices {
    let (root, run, notifier) = (self.root.clone(), self.run.id(), self.notifier.clone());
    let sink = Mutex::new(self.sink.another());
    let noted = Arc::new(move |problem: RunnerProblem| {
      runner_note::note((&root, &mut **sink.lock()), (run, invocation), problem, &notifier);
    });
    let (crash_noted, failure_noted) = (Arc::clone(&noted), Arc::clone(&noted));
    let tools = StepServices {
      services: Arc::clone(&self.services),
      state: self.services.register(invocation),
      root: self.root.clone(),
      project_id: self.project.id(),
      worktree: worktree.to_path_buf(),
      run: self.run.id(),
      computer: Arc::clone(&self.computer),
      inherited: std::env::vars_os().collect(),
      project: environment.clone(),
      crashed: Arc::new(move |crash| crash_noted(RunnerProblem::ServiceCrashed(crash.clone()))),
      start_failed: Arc::new(move |failure| failure_noted(RunnerProblem::ServiceStartFailed(Box::new(failure)))),
      // A note that cannot be written is logged by `runner_note`: the start waits all the same.
      waiting: Arc::new(move |wait| noted(RunnerProblem::WaitingForServiceSlots(Box::new(wait)))),
      who: Holding {
        project: self.project.id(),
        task: self.task.key().to_string(),
        run: self.run.number().get(),
        step: self.slot_step(execution),
      },
    };
    self.services.keep_tools(invocation, tools.clone());
    tools
  }

  /// The step `execution` executes, by its position in the run's pipeline and its name.
  fn slot_step(&self, execution: StepExecutionId) -> Option<SlotStep> {
    let key = self.run.execution(execution)?.step();
    let steps = self.run.snapshot().body().steps();
    let at = steps.iter().position(|step| &step.key == key)?;
    Some(SlotStep {
      number: u32::try_from(at + 1).ok()?,
      name: steps[at].name.to_string(),
    })
  }
}
