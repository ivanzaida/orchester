//! The prompt of an agent step's invocation, assembled for its injection event: for a new session
//! the whole prompt with the instructions injected at start, the outputs of the steps it runs after
//! (and, for Codex, the result schema); for a session that resumes the continue prompt with the
//! instructions injected at resume. A gated step a reviewer sent back is told what should change
//! instead (the plan revision). A new session of an attempt a failure sent back is told that
//! failure ([`super::retry`]).

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  resource::{InjectionEvent, Instruction, Step},
  revision::Revision,
  run::{
    AssembledPrompt, BaseMoved, DeliveredMessages, DependencyOutput, InstructionSource, InstructionVersion,
    PinnedVersion, PipelineInfo, Plan, PlanRevision, PromptAssemblyError, PromptInput, ReturnedWork, Run, RunContext,
    StepOutput, run_delivers,
  },
  store::{ActivityStore, ResourceStore, RunStore},
};

use super::{
  super::{super::run::RunTask, Continuation, Occasion},
  PromptParts, problem,
  retry::LoadedReport,
};

impl RunTask {
  /// The prompt of `execution`'s next invocation: role, instructions, step and task with the run's
  /// variables filled in, then the outputs of the passed executions of the steps it depends on (a
  /// gated one's approved plan), and what a reviewer asked to change in its own plan, if the
  /// invocation revises it.
  pub(super) fn prompt(&self, parts: PromptParts<'_>) -> Result<AssembledPrompt, RunnerProblem> {
    self.with_prompt(parts, |input, event| input.assemble(event))
  }

  /// What `assemble` makes of the prompt input of `parts` at its injection event: a new session's at
  /// [`InjectionEvent::Start`], a resumed one's at [`InjectionEvent::Resume`].
  pub(super) fn with_prompt<T>(
    &self,
    parts: PromptParts<'_>,
    assemble: impl FnOnce(&PromptInput<'_>, InjectionEvent) -> Result<T, PromptAssemblyError>,
  ) -> Result<T, RunnerProblem> {
    let PromptParts {
      execution,
      worktree,
      step,
      profile,
      continuation,
      placed,
      code_index,
      memory,
      working,
    } = parts;
    let (event, checked_out) = injection(continuation, worktree);
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
    let retry = self.retry_report(execution, &outputs)?;
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
    let input = PromptInput {
      project: &self.project,
      task: &self.task,
      run: Some(RunContext {
        run: &self.run,
        attempt: self.attempt_number(execution),
        checked_out: checked_out.as_ref(),
      }),
      pipeline: self.pipeline_info(&pipeline_name),
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
      base_moved: continuation.notice.as_ref().map(|notice| BaseMoved {
        notice,
        after_turn: matches!(continuation.occasion, Occasion::AfterTurn),
      }),
      memory,
      retry: self.returned(retry.as_ref(), execution),
      working,
    };
    assemble(&input, event).map_err(|PromptAssemblyError::UnavailableVariable { variable, .. }| {
      RunnerProblem::UnavailableVariable {
        variable: variable.name().to_owned(),
      }
    })
  }

  /// Why the work came back to `execution`'s step: the failure `retry` reports, else the note of a person who
  /// returned the work of the ended run to it.
  fn returned<'a>(&'a self, retry: Option<&'a LoadedReport>, execution: StepExecutionId) -> Option<ReturnedWork<'a>> {
    retry
      .map(|loaded| ReturnedWork::Retry(loaded.report(self)))
      .or_else(|| {
        self
          .run
          .returned_work(execution)
          .map(|(note, conflicts)| ReturnedWork::AfterEnd { note, conflicts })
      })
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

  /// The pipeline version the run pinned, named `name`, and what the run delivers, as prompts name them.
  fn pipeline_info<'a>(&self, name: &'a str) -> PipelineInfo<'a> {
    let pinned = self.run.snapshot().pipeline();
    PipelineInfo {
      id: pinned.resource,
      name,
      version: pinned.number,
      delivers: run_delivers(&self.task, &self.run),
    }
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

/// The event an invocation continuing as `continuation` is injected at: a new session's
/// [`InjectionEvent::Start`], a resumed one's [`InjectionEvent::Resume`] with the commit its worktree
/// has checked out.
fn injection(continuation: &Continuation, worktree: &Path) -> (InjectionEvent, Option<Revision>) {
  if continuation.session.is_some() {
    (InjectionEvent::Resume, checked_out(worktree))
  } else {
    (InjectionEvent::Start, None)
  }
}

/// The commit `worktree` has checked out, a resumed agent's `run.revision`: its latest checkpoint, or
/// a commit it made or merged itself since, which a clean worktree leaves without a checkpoint.
fn checked_out(worktree: &Path) -> Option<Revision> {
  match super::super::super::checkpoint::head(worktree) {
    Ok(revision) => Some(revision),
    // The variable falls back to the latest checkpoint; before the first one the step then fails
    // naming it, as at start.
    Err(error) => {
      tracing::warn!(%error, worktree = %worktree.display(), "the resumed agent's worktree head could not be read");
      None
    }
  }
}
