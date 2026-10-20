//! The prompt an instruction takes part in, previewed for a chosen task, agent step, run and
//! injection event (`RGk5C`, 05.02 `fZa3J`): the latest published versions a run started now would
//! pin, with the edited instruction's draft in place of its published version. Variables resolve
//! against the task and the chosen run; without a run, the run's variables are listed as
//! unavailable instead of being filled in.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  project::Project,
  resource::{
    AgentProfile, InjectionEvent, Instruction, Pipeline, ResourceBody, ResourceKind, Step, StepKey, StepKind, Variable,
    Version,
  },
  run::{Composition, InstructionSource, InstructionVersion, PipelineInfo, ProfileInfo, PromptInput, Run, RunContext},
  store::{ResourceStore, RunStore, TaskStore},
  task::Task,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// A task a preview can be made for, with its runs, newest last. A run whose record cannot be read
/// has no variables to resolve against, so it is not offered; its task page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewTask {
  pub task: Task,
  pub runs: Vec<Run>,
}

/// A published instruction, in library order.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PublishedInstruction {
  id: ResourceId,
  name: Arc<str>,
  slug: Arc<str>,
  version: Version<Instruction>,
}

/// What previews are made of, read together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewCatalog {
  project: Project,
  tasks: Vec<PreviewTask>,
  pipelines: BTreeMap<ResourceId, (Arc<str>, Version<Pipeline>)>,
  profiles: BTreeMap<ResourceId, Version<AgentProfile>>,
  instructions: Vec<PublishedInstruction>,
}

/// What the preview is for, as chosen; a choice that no longer exists falls back to the first
/// one that does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewChoice {
  pub task: Option<TaskId>,
  pub step: Option<StepKey>,
  pub run: Option<RunId>,
}

/// Why there is nothing to preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewProblem {
  NoTasks,
  /// The task has no pipeline, or its pipeline was never published.
  NoPublishedPipeline,
  /// No step of the pipeline runs a published agent profile.
  NoAgentSteps,
}

/// An agent step of the previewed pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentStep {
  pub key: StepKey,
  pub step: Arc<str>,
  pub role: Arc<str>,
}

/// The instruction being edited, as the preview substitutes it.
#[derive(Clone, Copy, Debug)]
pub struct EditedInstruction<'a> {
  pub id: ResourceId,
  pub name: &'a str,
  pub slug: &'a str,
  pub body: &'a Instruction,
  pub version: InstructionVersion,
}

impl PreviewCatalog {
  pub fn load(db: &ProjectDb) -> Result<Self, StoreError> {
    let mut tasks = Vec::new();
    for task in db.tasks()? {
      let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
      tasks.push(PreviewTask {
        // An unreadable run cannot be previewed; its task page and task row show why.
        runs: runs.into_iter().filter_map(Result::ok).collect(),
        task,
      });
    }
    let mut pipelines = BTreeMap::new();
    for resource in db.resources(ResourceKind::Pipeline)? {
      if let Some(version) = db.latest_version::<Pipeline>(resource.id())? {
        pipelines.insert(resource.id(), (resource.name().as_str().into(), version));
      }
    }
    let mut instructions = Vec::new();
    for resource in db.resources(ResourceKind::Instruction)? {
      if let Some(version) = db.latest_version::<Instruction>(resource.id())? {
        instructions.push(PublishedInstruction {
          id: resource.id(),
          name: resource.name().as_str().into(),
          slug: resource.slug().as_str().into(),
          version,
        });
      }
    }
    Ok(Self {
      project: db.project().clone(),
      tasks,
      pipelines,
      profiles: latest(db)?,
      instructions,
    })
  }

  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    Self::load(&ProjectDb::open(repo_root)?)
  }

  pub fn tasks(&self) -> &[PreviewTask] {
    &self.tasks
  }

  /// The preview `choice` stands for, or why there is none.
  pub fn resolve(&self, choice: &PreviewChoice) -> Result<Resolved<'_>, PreviewProblem> {
    let first = self.tasks.first().ok_or(PreviewProblem::NoTasks)?;
    let task = choice
      .task
      .and_then(|id| self.tasks.iter().find(|task| task.task.id() == id))
      .unwrap_or(first);
    let (pipeline_id, (pipeline_name, pipeline)) = task
      .task
      .details()
      .pipeline
      .and_then(|id| self.pipelines.get_key_value(&id))
      .ok_or(PreviewProblem::NoPublishedPipeline)?;
    let steps: Vec<(&Step, ResourceId, &Version<AgentProfile>)> = pipeline
      .body()
      .steps()
      .iter()
      .filter_map(|step| match &step.kind {
        StepKind::Agent {
          profile: Some(profile), ..
        } => self.profiles.get(profile).map(|version| (step, *profile, version)),
        _ => None,
      })
      .collect();
    let chosen = choice
      .step
      .as_ref()
      .and_then(|key| steps.iter().find(|(step, ..)| &step.key == key));
    let &(step, profile_id, profile) = chosen.or(steps.first()).ok_or(PreviewProblem::NoAgentSteps)?;
    let run = choice.run.and_then(|id| task.runs.iter().find(|run| run.id() == id));
    Ok(Resolved {
      catalog: self,
      task,
      pipeline: (*pipeline_id, pipeline_name, pipeline),
      steps: steps
        .iter()
        .map(|(step, _, profile)| AgentStep {
          key: step.key.clone(),
          step: step.name.as_str().into(),
          role: profile.body().role().as_str().into(),
        })
        .collect(),
      step,
      profile: (profile_id, profile),
      run,
    })
  }
}

fn latest<B: ResourceBody>(db: &ProjectDb) -> Result<BTreeMap<ResourceId, Version<B>>, StoreError> {
  let mut versions = BTreeMap::new();
  for resource in db.resources(B::KIND)? {
    if let Some(version) = db.latest_version::<B>(resource.id())? {
      versions.insert(resource.id(), version);
    }
  }
  Ok(versions)
}

/// A preview that can be made: the task, the agent step and the run it is for.
pub struct Resolved<'a> {
  catalog: &'a PreviewCatalog,
  pub task: &'a PreviewTask,
  pipeline: (ResourceId, &'a Arc<str>, &'a Version<Pipeline>),
  /// Every agent step of the pipeline, for choosing another one.
  pub steps: Vec<AgentStep>,
  pub step: &'a Step,
  profile: (ResourceId, &'a Version<AgentProfile>),
  pub run: Option<&'a Run>,
}

impl Resolved<'_> {
  /// Position in the published pipeline, including its non-agent steps.
  pub fn step_ordinal(&self) -> usize {
    self
      .pipeline
      .2
      .body()
      .steps()
      .iter()
      .position(|step| step.key == self.step.key)
      .expect("resolved step belongs to the published pipeline")
      + 1
  }
  pub fn pipeline_name(&self) -> &str {
    self.pipeline.1
  }

  pub fn role(&self) -> &str {
    self.profile.1.body().role().as_str()
  }

  /// The prompt at `event` with `edited` in place of its published version.
  pub fn compose(&self, edited: EditedInstruction<'_>, event: InjectionEvent) -> Composition {
    let mut sources: Vec<(&str, InstructionSource<'_>)> = self
      .catalog
      .instructions
      .iter()
      .filter(|published| published.id != edited.id)
      .map(|published| {
        let source = InstructionSource {
          resource: published.id,
          name: &published.name,
          version: InstructionVersion::Published {
            number: published.version.number(),
          },
          body: published.version.body(),
        };
        (&*published.slug, source)
      })
      .collect();
    let edited_source = InstructionSource {
      resource: edited.id,
      name: edited.name,
      version: edited.version,
      body: edited.body,
    };
    let position = sources.partition_point(|(slug, _)| *slug < edited.slug);
    sources.insert(position, (edited.slug, edited_source));
    let sources: Vec<InstructionSource<'_>> = sources.into_iter().map(|(_, source)| source).collect();
    self.input(&sources).compose(event)
  }

  /// The value `variable` has in this preview; `None` when it has none.
  pub fn value(&self, variable: Variable) -> Option<String> {
    self.input(&[]).value(variable)
  }

  fn input<'b>(&'b self, instructions: &'b [InstructionSource<'b>]) -> PromptInput<'b> {
    let (pipeline_id, pipeline_name, pipeline) = self.pipeline;
    let (profile_id, profile) = self.profile;
    let run = self.run.map(|run| RunContext {
      run,
      // A preview has no agent's worktree: a resumed agent's run.revision is its latest checkpoint.
      checked_out: None,
      attempt: run.latest_execution(&self.step.key).and_then(|execution| {
        run
          .attempts()
          .iter()
          .find(|attempt| attempt.execution(execution.id()).is_some())
          .map(|attempt| attempt.number())
      }),
    });
    PromptInput {
      project: &self.catalog.project,
      task: &self.task.task,
      run,
      pipeline: PipelineInfo {
        id: pipeline_id,
        name: pipeline_name,
        version: pipeline.number(),
      },
      step: self.step,
      profile: ProfileInfo {
        id: profile_id,
        version: profile.number(),
        body: profile.body(),
      },
      instructions,
      // Skills are listed with where the runner placed them, and the code index part names the index
      // the runner built or reused, both only while a run executes; the viewer's note says so.
      skills: &[],
      code_index: None,
      // Outputs of earlier steps exist only while a run executes.
      dependencies: &[],
      revision: None,
      messages: None,
      base_moved: None,
      // Memory is recalled for an invocation as it starts, from the records of that moment.
      memory: &[],
      retry: None,
      // What the agent's CLI gets depends on the connection a run starts it with.
      working: None,
    }
  }
}
