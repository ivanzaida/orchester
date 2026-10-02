//! The prompt of an agent step's invocation, assembled for its injection event: for a new session
//! the whole prompt with the instructions injected at start, the outputs of the steps it runs after
//! (and, for Codex, the result schema); for a session that resumes the continue prompt with the
//! instructions injected at resume. A gated step a reviewer sent back is told what should change
//! instead (the plan revision).

use std::num::NonZeroU32;

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  resource::{InjectionEvent, Instruction, Step},
  run::{
    AssembledPrompt, DeliveredMessages, DependencyOutput, InstructionSource, InstructionVersion, PinnedVersion,
    PipelineInfo, Plan, PlanRevision, PromptAssemblyError, PromptInput, Run, RunContext, StepOutput,
  },
  store::{ActivityStore, ResourceStore, RunStore},
};

use super::{
  super::{super::run::RunTask, Occasion},
  PromptParts, problem,
};

impl RunTask {
  /// The prompt of `execution`'s next invocation: role, instructions, step and task with the run's
  /// variables filled in, then the outputs of the passed executions of the steps it depends on (a
  /// gated one's approved plan), and what a reviewer asked to change in its own plan, if the
  /// invocation revises it.
  pub(super) fn prompt(&self, parts: PromptParts<'_>) -> Result<AssembledPrompt, RunnerProblem> {
    let PromptParts {
      execution,
      step,
      profile,
      continuation,
      placed,
      code_index,
      memory,
    } = parts;
    let event = if continuation.session.is_some() {
      InjectionEvent::Resume
    } else {
      InjectionEvent::Start
    };
    let pipeline = self.run.snapshot().pipeline();
    let pipeline_name = self.pipeline_name()?;
    let outputs = self
      .db
      .step_outputs(self.run.id())
      .map_err(|error| problem(error.to_string()))?;
    let plans = self
      .db
      .plans(self.run.id())
      .map_err(|error| problem(error.to_string()))?;
    let dependencies = dependencies(&self.run, step, &outputs, &plans);
    let instructions = self.pinned_instructions()?;
    let sources: Vec<InstructionSource<'_>> = instructions
      .iter()
      .map(|(pin, name, body)| InstructionSource {
        resource: pin.resource,
        name,
        version: InstructionVersion::Published { number: pin.number },
        body,
      })
      .collect();
    let skills = placed.listings();
    PromptInput {
      project: &self.project,
      task: &self.task,
      run: Some(RunContext {
        run: &self.run,
        attempt: self.attempt_number(execution),
      }),
      pipeline: PipelineInfo {
        id: pipeline.resource,
        name: &pipeline_name,
        version: pipeline.number,
      },
      step,
      profile,
      instructions: &sources,
      skills: &skills,
      code_index,
      dependencies: &dependencies,
      revision: Plan::awaiting_revision(&plans, execution, continuation.last_started).map(|(plan, note)| {
        PlanRevision {
          version: plan.version,
          plan: &plan.text,
          note,
        }
      }),
      messages: (!continuation.messages.is_empty() || matches!(continuation.occasion, Occasion::Recovery(_))).then(
        || DeliveredMessages {
          occasion: continuation.occasion.as_prompt(),
          messages: &continuation.messages,
        },
      ),
      memory,
    }
    .assemble(event)
    .map_err(
      |PromptAssemblyError::UnavailableVariable { variable, .. }| RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      },
    )
  }

  /// The number of the attempt `execution` belongs to.
  fn attempt_number(&self, execution: StepExecutionId) -> Option<NonZeroU32> {
    self
      .run
      .attempts()
      .iter()
      .find(|attempt| attempt.execution(execution).is_some())
      .map(|attempt| attempt.number())
  }

  /// The name of the run's pipeline, as prompts name it.
  fn pipeline_name(&self) -> Result<String, RunnerProblem> {
    self
      .db
      .resource(self.run.snapshot().pipeline().resource)
      .map_err(|error| problem(error.to_string()))?
      .map(|resource| resource.name().as_str().to_owned())
      .ok_or_else(|| problem("the run's pipeline no longer exists".to_owned()))
  }

  /// The instruction versions the run pinned, with their names, in library (slug) order.
  fn pinned_instructions(&self) -> Result<Vec<(PinnedVersion, String, Instruction)>, RunnerProblem> {
    let mut pinned = Vec::new();
    for pin in self.run.snapshot().instructions() {
      let resource = self
        .db
        .resource(pin.resource)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("instruction {} no longer exists", pin.resource)))?;
      let version = self
        .db
        .version::<Instruction>(pin.resource, pin.number)
        .map_err(|error| problem(error.to_string()))?
        .ok_or_else(|| problem(format!("instruction version {} is missing", pin.number)))?;
      pinned.push((
        resource.slug().clone(),
        (*pin, resource.name().as_str().to_owned(), version.body().clone()),
      ));
    }
    pinned.sort_by(|(left, _), (right, _)| left.cmp(right));
    Ok(pinned.into_iter().map(|(_, instruction)| instruction).collect())
  }
}

/// The outputs of the passed executions of the steps `step` depends on, in pipeline order: a gated
/// step's is the plan that was approved, its latest; any other's what it left for the steps after
/// it.
fn dependencies<'a>(
  run: &'a Run,
  step: &Step,
  outputs: &'a [StepOutput],
  plans: &'a [Plan],
) -> Vec<DependencyOutput<'a>> {
  run
    .snapshot()
    .body()
    .steps()
    .iter()
    .filter(|dependency| step.depends_on.contains(&dependency.key))
    .filter_map(|dependency| {
      let passed = run.latest_execution(&dependency.key)?;
      let attempt = run
        .attempts()
        .iter()
        .find(|attempt| attempt.execution(passed.id()).is_some())?;
      let text = Plan::output_of(passed.id(), outputs, plans).unwrap_or_default();
      Some(DependencyOutput {
        step: dependency,
        attempt: attempt.number(),
        text,
      })
    })
    .collect()
}
