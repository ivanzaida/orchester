//! The team's configuration as the fake server publishes it, and the schema-2 execution bundle it builds from it at a
//! work start, with the task's number and each pinned version's label: a Builder profile run by Codex, a one-step
//! pipeline and a skill with one reference file. Shared with the runner's team start tests (`#[path]`), so both drive
//! the same bundle.

#![allow(dead_code, reason = "each test binary uses some of the fixture")]

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_api::{
  SharedLimits,
  v2::{BundleStart, ExecutionBundle, Subject, TaskAtStart},
};
use orchester_core::{
  connection::AgentProvider,
  id::{ClaimId, ConnectionId, ModuleId, ProjectId, ResourceId, VersionId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  resource::{
    AgentProfile, Deliverable, InstructionScope, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess,
    Skill, SkillFile, SkillFilePath, Slug, Step, StepKey, StepKind, Version, VersionNumber,
  },
  run::{
    BranchName, CommitId, PublishedVersions, RunSnapshot,
    bundle::{CanonicalExecutionInputs, PinnedBodies, ResourceLabel, file_sha256},
  },
  sharing::TaskRevision,
  time::Timestamp,
};

/// The bytes of the skill's one reference file.
pub const NOTES: &[u8] = b"Run the checks before you finish.";
/// The task's number on the server.
pub const TASK_NUMBER: u32 = 7;
/// The skill's slug, the folder its files are placed in.
pub const SKILL_SLUG: &str = "checks";

fn name(value: &str) -> Name {
  Name::new(value).expect("name")
}

fn label(slug: &str, title: &str) -> ResourceLabel {
  ResourceLabel::new(Slug::new(slug).expect("slug"), name(title))
}

/// The team's published versions, pinned.
#[derive(Clone)]
pub struct Config {
  pub profile: ResourceId,
  pub pipeline: ResourceId,
  pub skill: ResourceId,
  pub inputs: CanonicalExecutionInputs,
  /// The CLI the bundle binds the profile to.
  pub cli: AgentProvider,
}

impl Config {
  /// The configuration; `instructions` is the Implement step's prompt.
  pub fn new(instructions: &str) -> Self {
    let (profile, pipeline, skill) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());
    let profile_version = builder();
    let pipeline_version = one_step(profile, instructions);
    let skill_version = checks();
    let profiles = BTreeMap::from([(profile, profile_version.clone())]);
    let skills = BTreeMap::from([(skill, skill_version.clone())]);
    let published = PublishedVersions {
      profiles: &profiles,
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &skills,
    };
    let snapshot = RunSnapshot::build(pipeline, &pipeline_version, published).expect("snapshot");
    let bodies = PinnedBodies {
      labels: &[
        (profile_version.id(), label("builder", "Builder")),
        (pipeline_version.id(), label("delivery", "Delivery")),
        (skill_version.id(), label(SKILL_SLUG, "Checks")),
      ],
      profiles: &[profile_version],
      instructions: &[],
      tools: &[],
      skills: &[skill_version],
    };
    Self {
      profile,
      pipeline,
      skill,
      inputs: CanonicalExecutionInputs::pin(&snapshot, bodies).expect("inputs"),
      cli: AgentProvider::CodexCli,
    }
  }

  /// This configuration whose profile runs on `cli`.
  pub fn running(self, cli: AgentProvider) -> Self {
    Self { cli, ..self }
  }

  /// The bundle the server builds for `claim` on `module`'s branch from `base_commit`.
  pub fn bundle(
    &self,
    (project, task): (ProjectId, Subject),
    claim: ClaimId,
    module: ModuleId,
    base_commit: &str,
  ) -> ExecutionBundle {
    let start = BundleStart {
      project_id: project,
      subject: task,
      claim_id: claim,
      module_id: module,
      branch: BranchName::new("module/webhooks").expect("branch"),
      base_commit: CommitId::new(base_commit).expect("commit"),
      definition_revision: TaskRevision::FIRST,
      task_number: NonZeroU32::new(TASK_NUMBER).expect("a number"),
      task: TaskAtStart {
        title: "Say hello".into(),
        brief: "Append one line.".into(),
        criteria: vec!["README.md ends with hello".into()],
      },
      limits: SharedLimits {
        budget_tokens: None,
        max_duration_ms: None,
      },
      delivery: Deliverable::default(),
    };
    ExecutionBundle::new(start, self.inputs.clone(), &[(self.profile, self.cli)]).expect("bundle")
  }
}

/// The Builder profile, version 1, on the publisher's connection.
pub fn builder() -> Version<AgentProfile> {
  let body = AgentProfile::new(
    name("Builder"),
    "Implements tasks.".into(),
    PromptText::new("Work carefully.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::Off,
  ));
  Version::new(VersionId::generate(), VersionNumber::FIRST, body, Timestamp::now())
}

/// The pipeline, version 1: Implement, run by `profile`.
pub fn one_step(profile: ResourceId, instructions: &str) -> Version<Pipeline> {
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: name("Implement"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new(instructions).expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step]).expect("pipeline");
  Version::new(VersionId::generate(), VersionNumber::FIRST, pipeline, Timestamp::now())
}

/// The Checks skill, version 1, with one reference file.
fn checks() -> Version<Skill> {
  let file = SkillFile {
    path: SkillFilePath::new("references/notes.md").expect("path"),
    content: file_sha256(NOTES),
    size: NOTES.len() as u64,
  };
  let skill = Skill::new(
    "Runs the checks.".into(),
    "Run them.".into(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("skill");
  Version::new(VersionId::generate(), VersionNumber::FIRST, skill, Timestamp::now())
}
