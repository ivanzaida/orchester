//! The prompt an agent invocation starts with: its parts in assembly order, the instructions
//! injected at each event, where every part came from, and variables without a value.

use std::{collections::BTreeSet, num::NonZeroU32};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentProfile, AgentTargets, Exclusion, InjectionEvent, Instruction, InstructionScope, PromptText, ProviderSettings,
    Step, Variable, VersionNumber,
  },
  run::{
    CONTINUE_PROMPT, DependencyOutput, InstructionSource, InstructionVersion, PipelineInfo, ProfileInfo,
    PromptAssemblyError, PromptInput, PromptPart, PromptSource, RunContext,
  },
  task::Task,
};

use crate::fixture::{Config, Logged, at, key, model, project, revision, task};

pub(crate) fn builder(instructions: &str) -> AgentProfile {
  AgentProfile::new(
    Name::new("Builder").expect("valid role"),
    "Implements the approved plan.".into(),
    PromptText::new(instructions).expect("valid prompt"),
    orchester_core::id::ConnectionId::generate(),
    model("gpt-5.5"),
    ProviderSettings::default(),
  )
}

pub(crate) fn step_with(instructions: &str) -> Step {
  let config = Config::feature_delivery();
  let mut step = config
    .pipeline_version
    .body()
    .step(&key("implement"))
    .expect("step")
    .clone();
  step.instructions = PromptText::new(instructions).expect("valid prompt");
  step
}

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(value).expect("positive"))
}

fn instruction(text: &str, targets: AgentTargets, events: &[InjectionEvent], scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    targets,
    events.iter().copied().collect(),
    scope,
  )
  .expect("valid instruction")
}

const BOTH: [InjectionEvent; 2] = [InjectionEvent::Start, InjectionEvent::Resume];

/// Everything but the instructions, for the implement step of Feature delivery v3 run by Builder v2.
pub(crate) struct Prompt {
  project: orchester_core::project::Project,
  task: Task,
  logged: Logged,
  pipeline: ResourceId,
  profile_id: ResourceId,
  profile: AgentProfile,
  step: Step,
}

impl Prompt {
  pub(crate) fn new(profile: AgentProfile, step: Step, task: Task) -> Self {
    Self {
      project: project(),
      task,
      logged: Logged::started(),
      pipeline: ResourceId::generate(),
      profile_id: ResourceId::generate(),
      profile,
      step,
    }
  }

  pub(crate) fn input<'a>(
    &'a self,
    instructions: &'a [InstructionSource<'a>],
    dependencies: &'a [DependencyOutput<'a>],
    run: bool,
  ) -> PromptInput<'a> {
    PromptInput {
      project: &self.project,
      task: &self.task,
      run: run.then_some(RunContext {
        run: &self.logged.run,
        attempt: Some(NonZeroU32::MIN),
      }),
      pipeline: PipelineInfo {
        id: self.pipeline,
        name: "Feature delivery",
        version: number(3),
      },
      step: &self.step,
      profile: ProfileInfo {
        id: self.profile_id,
        version: number(2),
        body: &self.profile,
      },
      instructions,
      skills: &[],
      code_index: None,
      dependencies,
      revision: None,
      messages: None,
      memory: &[],
      retry: None,
      working: None,
    }
  }
}

fn source<'a>(resource: ResourceId, name: &'a str, body: &'a Instruction) -> InstructionSource<'a> {
  InstructionSource {
    resource,
    name,
    version: InstructionVersion::Published { number: number(1) },
    body,
  }
}

#[test]
fn prompt_assembles_role_step_and_task_in_order() {
  let prompt = Prompt::new(
    builder("Keep commits small for {{task.id}}."),
    step_with("Work on {{run.branch}}, attempt {{ run.attempt }}. Literal \\{{task.title}}."),
    task(
      "Users sign in with GitHub.",
      &["Sign-in works", "Cancel shows an error"],
    ),
  );
  let assembled = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("every variable has a value");
  let expected = "# Your role: Builder\n\nImplements the approved plan.\n\nKeep commits small for ORC-12.\n\n# Current \
                  step: implement\n\nWork on orchester/orc-129, attempt 1. Literal {{task.title}}.\n\n# Task ORC-12: \
                  Add GitHub sign-in\n\nUsers sign in with GitHub.\n\n## Acceptance criteria\n- Sign-in works\n- \
                  Cancel shows an error\n\n";
  let (context, _) = assembled
    .text
    .split_once("# When you finish\n")
    .expect("result contract");
  assert_eq!(context, expected);
  let parts: Vec<PromptPart> = assembled.sources.sources.iter().map(PromptSource::part).collect();
  assert_eq!(parts, [PromptPart::AgentRole, PromptPart::Step, PromptPart::Task]);
  assert_eq!(assembled.sources.event, InjectionEvent::Start);
}

#[test]
fn prompt_names_a_variable_without_a_value_instead_of_leaving_it_empty() {
  let prompt = Prompt::new(
    builder("Fine."),
    step_with("See {{task.attachments}}."),
    task("Brief.", &[]),
  );
  let error = prompt.input(&[], &[], true).assemble(InjectionEvent::Start);
  assert_eq!(
    error,
    Err(PromptAssemblyError::UnavailableVariable {
      variable: Variable::TaskAttachments,
      section: PromptSource::Step {
        pipeline: prompt.pipeline,
        version: number(3),
        step: key("implement"),
      },
    })
  );
  // The budget in force, as a whole number of tokens.
  let budget = Prompt::new(builder("Budget {{run.budget}}"), step_with(""), task("Brief.", &[]));
  let text = budget
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("known")
    .text;
  assert!(text.contains("Budget 1400000"));
}

#[test]
fn prompt_lists_the_outputs_of_the_steps_it_runs_after_with_their_sources() {
  let config = Config::feature_delivery();
  let body = config.pipeline_version.body();
  let (checks, review) = (
    body.step(&key("checks")).expect("step"),
    body.step(&key("review")).expect("step"),
  );
  let pr = body.step(&key("pr")).expect("step").clone();
  let prompt = Prompt::new(builder("Fine."), pr, task("Brief.", &[]));
  let attempt = NonZeroU32::new(2).expect("positive");
  let dependencies = [
    DependencyOutput {
      step: checks,
      attempt,
      text: "All 38 tests pass.",
    },
    DependencyOutput {
      step: review,
      attempt,
      text: " ",
    },
  ];
  let text = prompt
    .input(&[], &dependencies, true)
    .assemble(InjectionEvent::Start)
    .expect("every variable has a value")
    .text;
  let results = text.find("# Results of earlier steps").expect("results section");
  assert!(results > text.find("# Task ORC-12").expect("task section"));
  assert!(text.contains("\nSources: checks (`checks`, attempt 2), review (`review`, attempt 2)\n"));
  assert!(text.contains("\n## checks (`checks`, attempt 2)\n\nAll 38 tests pass.\n"));
  assert!(text.contains("\n## review (`review`, attempt 2)\n\n(no output)\n"));
  let contract = text.find("# When you finish\n").expect("result contract");
  assert!(contract > results);
  assert!(text[contract..].contains("resumes the same session"));
}

/// One instruction per scope, given in an order unlike assembly order.
struct Library {
  ids: [ResourceId; 4],
  bodies: [Instruction; 4],
}

impl Library {
  fn new(prompt: &Prompt, events: &[InjectionEvent]) -> Self {
    let step = InstructionScope::Step {
      pipeline: prompt.pipeline,
      step: key("implement"),
    };
    let role = InstructionScope::AgentRole {
      role: Name::new("Builder").expect("valid role"),
    };
    Self {
      ids: [(); 4].map(|()| ResourceId::generate()),
      bodies: [
        instruction("Test what you change.", AgentTargets::All, events, step),
        instruction(
          "Commit with {{task.id}}.",
          AgentTargets::All,
          events,
          InstructionScope::Project,
        ),
        instruction(
          "Stay on the plan.",
          AgentTargets::All,
          events,
          InstructionScope::Pipeline {
            pipeline: prompt.pipeline,
          },
        ),
        instruction("Prefer small diffs.", AgentTargets::All, events, role),
      ],
    }
  }

  fn sources(&self) -> Vec<InstructionSource<'_>> {
    let names = ["Step checks", "Git conventions", "Delivery rules", "Builder habits"];
    self
      .ids
      .iter()
      .zip(&self.bodies)
      .zip(names)
      .map(|((id, body), name)| source(*id, name, body))
      .collect()
  }
}

#[test]
fn prompt_injects_instructions_in_assembly_order() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let library = Library::new(&prompt, &[InjectionEvent::Start]);
  let sources = library.sources();
  let assembled = prompt
    .input(&sources, &[], true)
    .assemble(InjectionEvent::Start)
    .expect("every variable has a value");
  let text = &assembled.text;
  let order = [
    "# Your role: Builder",
    "Role.",
    "## Builder habits\n\nPrefer small diffs.",
    "# Project rules\n\n## Git conventions\n\nCommit with ORC-12.",
    "# Pipeline instructions: Feature delivery\n\n## Delivery rules\n\nStay on the plan.",
    "# Current step: implement\n\nStep.",
    "## Step checks\n\nTest what you change.",
    "# Task ORC-12",
  ];
  let positions: Vec<usize> = order
    .iter()
    .map(|part| {
      text
        .find(part)
        .unwrap_or_else(|| panic!("`{part}` is missing from:\n{text}"))
    })
    .collect();
  assert!(positions.is_sorted(), "{text}");
  let [step_rule, project_rule, pipeline_rule, role_rule] = library.ids;
  let instruction_sources: Vec<(ResourceId, PromptPart)> = assembled
    .sources
    .sources
    .iter()
    .filter_map(|source| match source {
      PromptSource::Instruction { resource, part, .. } => Some((*resource, *part)),
      _ => None,
    })
    .collect();
  assert_eq!(
    instruction_sources,
    [
      (role_rule, PromptPart::AgentRole),
      (project_rule, PromptPart::ProjectRules),
      (pipeline_rule, PromptPart::Pipeline),
      (step_rule, PromptPart::Step),
    ]
  );
  assert_eq!(assembled.sources.sources.len(), 7);
}

#[test]
fn prompt_leaves_out_instructions_for_other_agents_steps_pipelines_and_events() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let reviewer = BTreeSet::from([Name::new("Reviewer").expect("valid role")]);
  let other_step = InstructionScope::Step {
    pipeline: prompt.pipeline,
    step: key("review"),
  };
  let bodies = [
    instruction(
      "Review.",
      AgentTargets::Roles(reviewer),
      &BOTH,
      InstructionScope::Project,
    ),
    instruction("Elsewhere.", AgentTargets::All, &BOTH, other_step),
    instruction(
      "Other pipeline.",
      AgentTargets::All,
      &BOTH,
      InstructionScope::Pipeline {
        pipeline: ResourceId::generate(),
      },
    ),
    instruction(
      "Resume only.",
      AgentTargets::All,
      &[InjectionEvent::Resume],
      InstructionScope::Project,
    ),
    instruction(
      "This profile.",
      AgentTargets::Profiles(BTreeSet::from([prompt.profile_id])),
      &BOTH,
      InstructionScope::Project,
    ),
    instruction(
      "Another profile.",
      AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
      &BOTH,
      InstructionScope::Project,
    ),
    instruction(
      "Reviewer role.",
      AgentTargets::All,
      &BOTH,
      InstructionScope::AgentRole {
        role: Name::new("Reviewer").expect("valid role"),
      },
    ),
  ];
  let ids: Vec<ResourceId> = bodies.iter().map(|_| ResourceId::generate()).collect();
  let sources: Vec<InstructionSource<'_>> = ids
    .iter()
    .zip(&bodies)
    .map(|(id, body)| source(*id, "Rule", body))
    .collect();
  let composition = prompt.input(&sources, &[], true).compose(InjectionEvent::Start);
  let reasons: Vec<(ResourceId, Exclusion)> = composition
    .excluded()
    .iter()
    .map(|excluded| (excluded.resource, excluded.reason))
    .collect();
  assert_eq!(
    reasons,
    [
      (ids[0], Exclusion::OtherAgents),
      (ids[1], Exclusion::OtherStep),
      (ids[2], Exclusion::OtherPipeline),
      (ids[3], Exclusion::OtherEvent),
      (ids[5], Exclusion::OtherAgents),
      (ids[6], Exclusion::OtherRole),
    ]
  );
  assert!(composition.text().contains("This profile."));
  assert!(!composition.text().contains("Resume only."));
}

#[test]
fn prompt_for_a_resumed_session_is_the_continue_prompt_with_resume_instructions() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let plain = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Resume)
    .expect("no variables");
  assert_eq!(
    plain
      .text
      .split_once("\n\n# When you finish\n")
      .expect("result contract")
      .0,
    CONTINUE_PROMPT
  );
  assert!(plain.sources.sources.is_empty());

  let start_only = instruction(
    "Read the plan first.",
    AgentTargets::All,
    &[InjectionEvent::Start],
    InstructionScope::Project,
  );
  let resume = instruction(
    "Check `git status` on {{run.branch}} before you continue.",
    AgentTargets::All,
    &[InjectionEvent::Resume],
    InstructionScope::Project,
  );
  let ids = [ResourceId::generate(), ResourceId::generate()];
  let sources = [
    source(ids[0], "Start rule", &start_only),
    source(ids[1], "Resume rule", &resume),
  ];
  let assembled = prompt
    .input(&sources, &[], true)
    .assemble(InjectionEvent::Resume)
    .expect("every variable has a value");
  assert_eq!(
    assembled
      .text
      .split_once("\n\n# When you finish\n")
      .expect("result contract")
      .0,
    format!(
      "{CONTINUE_PROMPT}\n\n# Project rules\n\n## Resume rule\n\nCheck `git status` on orchester/orc-129 before you \
       continue.\n"
    )
  );
  assert_eq!(assembled.sources.event, InjectionEvent::Resume);
  assert_eq!(
    assembled.sources.sources,
    [PromptSource::Instruction {
      resource: ids[1],
      part: PromptPart::ProjectRules,
      version: InstructionVersion::Published { number: number(1) },
    }]
  );
}

#[test]
fn prompt_preview_keeps_placeholders_without_values_and_lists_them() {
  let prompt = Prompt::new(builder("Role."), step_with("Step."), task("Brief.", &[]));
  let rule = instruction(
    "Work on {{run.branch}} for {{task.title}}.",
    AgentTargets::All,
    &BOTH,
    InstructionScope::Project,
  );
  let id = ResourceId::generate();
  let draft = [InstructionSource {
    resource: id,
    name: "Branch rule",
    version: InstructionVersion::Draft {
      based_on: Some(number(2)),
    },
    body: &rule,
  }];
  // A preview for a run that does not exist yet: the run's variables have no value.
  let composition = prompt.input(&draft, &[], false).compose(InjectionEvent::Start);
  let section = composition
    .sections()
    .iter()
    .find(|section| section.name == "Branch rule")
    .expect("the draft is part of the prompt");
  assert_eq!(section.text, "Work on {{run.branch}} for Add GitHub sign-in.");
  assert_eq!(section.variables, [Variable::RunBranch, Variable::TaskTitle]);
  assert_eq!(section.unavailable, [Variable::RunBranch]);
  assert_eq!(
    composition.into_prompt(),
    Err(PromptAssemblyError::UnavailableVariable {
      variable: Variable::RunBranch,
      section: PromptSource::Instruction {
        resource: id,
        part: PromptPart::ProjectRules,
        version: InstructionVersion::Draft {
          based_on: Some(number(2)),
        },
      },
    })
  );
}

#[test]
fn prompt_run_revision_is_the_latest_checkpoint_and_the_base_where_the_run_started() {
  let mut prompt = Prompt::new(
    builder("Role."),
    step_with("From {{run.base_revision}} to {{run.revision}}."),
    task("Brief.", &[]),
  );
  let base = revision('a');
  assert_eq!(
    prompt.input(&[], &[], true).value(Variable::RunBaseRevision).as_deref(),
    Some(base.as_str())
  );
  // Before the run's first checkpoint there is no commit to name.
  assert_eq!(
    prompt.input(&[], &[], true).assemble(InjectionEvent::Start),
    Err(PromptAssemblyError::UnavailableVariable {
      variable: Variable::RunRevision,
      section: PromptSource::Step {
        pipeline: prompt.pipeline,
        version: number(3),
        step: key("implement"),
      },
    })
  );
  let research = prompt.logged.exec("research");
  prompt.logged.apply(|run| run.start_step(research, at(12)));
  for (checkpoint, time) in [('b', 13), ('c', 14)] {
    prompt
      .logged
      .apply(|run| run.record_checkpoint(research, revision(checkpoint), at(time)));
    let text = prompt
      .input(&[], &[], true)
      .assemble(InjectionEvent::Start)
      .expect("the run has a checkpoint")
      .text;
    let expected = format!("From {} to {}.", base.as_str(), revision(checkpoint).as_str());
    assert!(text.contains(&expected), "{text}");
  }
}
