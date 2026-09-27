//! The project's instructions as the Instructions screen reads and saves them, the editor's
//! choices, and the preview of the prompt a rule takes part in, with its draft in place of its
//! published version.

use std::{collections::BTreeSet, fs, time::Duration};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  permission::StepPermissions,
  project::ProjectKey,
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind, Variable,
  },
  run::{InstructionVersion, PromptPart, PromptSource},
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_desktop::{
  agents::data::VersionState,
  drafts::save_draft_in,
  instructions::{
    data::{NewInstruction, ProjectInstructions, create_in, publish_in},
    form::{Events, InstructionForm, ScopeChoice, Target},
    preview::{EditedInstruction, PreviewCatalog, PreviewChoice, PreviewProblem},
  },
};
use orchester_store::ProjectDb;
use tempfile::TempDir;

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// Kontur with the published Builder and Reviewer profiles and the published pipeline Delivery
/// (Implement by Builder, then Review by Reviewer).
struct Project {
  repo: TempDir,
  builder: ResourceId,
  pipeline: ResourceId,
}

fn profile(db: &mut ProjectDb, role: &str) -> ResourceId {
  let body = AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("Work carefully.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let slug = Slug::new(&role.to_lowercase()).expect("slug");
  let id = db
    .create_resource(slug, name(role), &body, at(1))
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(id, at(2)).expect("publish");
  id
}

fn step(key: &str, profile: ResourceId, depends_on: &[&str]) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(
      &key[..1]
        .to_uppercase()
        .chars()
        .chain(key.chars().skip(1))
        .collect::<String>(),
    ),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Do the step.").expect("prompt"),
    depends_on: depends_on.iter().map(|key| StepKey::new(key).expect("key")).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

impl Project {
  fn new() -> Self {
    let repo = tempfile::tempdir().expect("temporary directory");
    fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = orchester_core::project::Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("create project");
    let builder = profile(&mut db, "Builder");
    let reviewer = profile(&mut db, "Reviewer");
    let body = Pipeline::new(vec![
      step("implement", builder, &[]),
      step("review", reviewer, &["implement"]),
    ])
    .expect("pipeline");
    let pipeline = db
      .create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &body, at(3))
      .expect("pipeline")
      .id();
    db.publish_draft::<Pipeline>(pipeline, at(4)).expect("publish");
    let details = TaskDetails {
      title: name("Add GitHub sign-in"),
      brief: "Users sign in with GitHub.".into(),
      acceptance_criteria: Vec::new(),
      pipeline: Some(pipeline),
      limits: Some(
        TaskLimits::new(TokenBudget::new(1_000_000).expect("budget"), Duration::from_secs(1_800)).expect("limits"),
      ),
    };
    db.create_tasks(IdempotencyKey::generate(), vec![details], at(5))
      .expect("task");
    Self {
      repo,
      builder,
      pipeline,
    }
  }

  /// A published instruction with `text` for `targets`, injected at start.
  fn published(&self, slug: &str, text: &str, targets: AgentTargets) -> ResourceId {
    let mut db = ProjectDb::open(self.repo.path()).expect("open");
    let body = Instruction::new(
      PromptText::new(text).expect("prompt"),
      targets,
      BTreeSet::from([InjectionEvent::Start]),
      InstructionScope::Project,
    )
    .expect("instruction");
    let id = db
      .create_resource(Slug::new(slug).expect("slug"), name(slug), &body, at(6))
      .expect("create")
      .id();
    db.publish_draft::<Instruction>(id, at(7)).expect("publish");
    id
  }
}

#[test]
fn instructions_are_created_as_empty_drafts_saved_and_published() {
  let project = Project::new();
  let root = project.repo.path();
  let id = create_in(
    root,
    NewInstruction {
      name: name("Git conventions"),
      scope: InstructionScope::Project,
    },
  )
  .expect("create");
  let loaded = ProjectInstructions::load_in(root).expect("load");
  let entry = loaded.entry(id).expect("listed");
  assert_eq!(entry.state, VersionState::Draft { published: None });
  assert!(entry.body.text().is_blank());
  assert_eq!(&*entry.slug, "git-conventions");
  // A blank rule is not published.
  assert!(publish_in(root, id).is_err());

  let body = InstructionForm {
    text: "Name commits after {{task.id}}.",
    targets: &[Target::Role(name("Builder"))],
    events: Events::StartAndResume,
    scope: &ScopeChoice::Project,
  }
  .instruction()
  .expect("valid form");
  let draft = orchester_core::resource::Draft::new(body.clone(), None, at(10));
  save_draft_in(root, id, &draft).expect("save");
  let (version, _) = publish_in(root, id).expect("publish");
  assert_eq!(version.get(), 1);
  let loaded = ProjectInstructions::load_in(root).expect("load");
  let entry = loaded.entry(id).expect("listed");
  assert_eq!(entry.state, VersionState::Published(version));
  assert_eq!(entry.body, body);
  assert_eq!(loaded.roles(), BTreeSet::from([name("Builder"), name("Reviewer")]));
  assert_eq!(loaded.pipeline(project.pipeline).expect("pipeline").steps.len(), 2);
}

#[test]
fn instructions_apply_to_choices_keep_one_kind_the_newest_decides() {
  let (builder, reviewer, profile) = (
    Target::Role(name("Builder")),
    Target::Role(name("Reviewer")),
    Target::Profile(ResourceId::generate()),
  );
  let roles = vec![builder.clone(), reviewer.clone()];
  let only_builder = Target::after_change(&[Target::All], vec![Target::All, builder.clone()]);
  assert_eq!(only_builder, [builder]);
  assert_eq!(
    Target::after_change(&roles, [roles.clone(), vec![Target::All]].concat()),
    [Target::All]
  );
  let only_profile = Target::after_change(&roles, [roles.clone(), vec![profile.clone()]].concat());
  assert_eq!(only_profile, [profile]);
  assert_eq!(Target::after_change(&roles, vec![reviewer.clone()]), [reviewer]);
  let none = InstructionForm {
    text: "Rule.",
    targets: &[],
    events: Events::Start,
    scope: &ScopeChoice::Project,
  }
  .instruction()
  .expect_err("no agents chosen");
  assert!(none.settings.is_some());
  let role_rule = InstructionForm {
    text: "Rule.",
    targets: &[],
    events: Events::Resume,
    scope: &ScopeChoice::AgentRole(Some(name("Builder"))),
  }
  .instruction()
  .expect("a role rule applies to its role");
  assert_eq!(role_rule.targets(), &AgentTargets::All);
  let unknown = InstructionForm {
    text: "Use {{secret.token}}.",
    targets: &[Target::All],
    events: Events::Start,
    scope: &ScopeChoice::Project,
  }
  .instruction()
  .expect_err("unknown variable");
  assert!(unknown.text.is_some());
}

#[test]
fn instructions_preview_substitutes_the_draft_and_resolves_variables() {
  let project = Project::new();
  let root = project.repo.path();
  let reviewer_only = project.published(
    "review-checklist",
    "Check the tests.",
    AgentTargets::Roles(BTreeSet::from([name("Reviewer")])),
  );
  let edited_id = project.published("git-conventions", "Old text.", AgentTargets::All);
  let catalog = PreviewCatalog::load_in(root).expect("catalog");
  // The first agent step: Builder on Implement, no run yet.
  let resolved = catalog.resolve(&PreviewChoice::default()).expect("a preview");
  assert_eq!(resolved.role(), "Builder");
  assert_eq!(resolved.steps.len(), 2);
  let draft = Instruction::new(
    PromptText::new("Commit {{task.id}} on {{run.branch}}.").expect("prompt"),
    AgentTargets::Profiles(BTreeSet::from([project.builder])),
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let edited = EditedInstruction {
    id: edited_id,
    name: "Git conventions",
    slug: "git-conventions",
    body: &draft,
    version: InstructionVersion::Draft {
      based_on: Some(orchester_core::resource::VersionNumber::FIRST),
    },
  };
  let composition = resolved.compose(edited, InjectionEvent::Start);
  let rules: Vec<_> = composition
    .sections()
    .iter()
    .filter(|section| section.source.part() == PromptPart::ProjectRules)
    .collect();
  assert_eq!(rules.len(), 1);
  assert_eq!(rules[0].text, "Commit ORC-1 on {{run.branch}}.");
  assert_eq!(rules[0].unavailable, [Variable::RunBranch]);
  assert!(matches!(
    rules[0].source,
    PromptSource::Instruction {
      version: InstructionVersion::Draft { .. },
      ..
    }
  ));
  assert_eq!(composition.excluded().len(), 1);
  assert_eq!(composition.excluded()[0].resource, reviewer_only);
  assert_eq!(
    resolved.value(Variable::TaskTitle).as_deref(),
    Some("Add GitHub sign-in")
  );
  assert_eq!(resolved.value(Variable::RunId), None);

  // The Reviewer's step gets the checklist and not the Builder's rule.
  let review = PreviewChoice {
    step: Some(StepKey::new("review").expect("key")),
    ..PreviewChoice::default()
  };
  let reviewer = catalog.resolve(&review).expect("a preview");
  let composition = reviewer.compose(edited, InjectionEvent::Start);
  assert!(composition.text().contains("Check the tests."));
  assert!(!composition.text().contains("Commit ORC-1"));
}

#[test]
fn instructions_preview_says_why_there_is_nothing_to_preview() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = orchester_core::project::Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    name("Kontur"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let catalog = PreviewCatalog::load(&db).expect("catalog");
  assert_eq!(
    catalog.resolve(&PreviewChoice::default()).err(),
    Some(PreviewProblem::NoTasks)
  );
  let details = TaskDetails {
    title: name("Draft task"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: None,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], at(1))
    .expect("task");
  let catalog = PreviewCatalog::load(&db).expect("catalog");
  assert_eq!(
    catalog.resolve(&PreviewChoice::default()).err(),
    Some(PreviewProblem::NoPublishedPipeline)
  );
}
