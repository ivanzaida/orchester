//! The migration by which a local profile owns what its agents are given (`docs/design/team-configuration.md`, "Decided
//! 2026-11-09"): a database before it, whose instructions, skills and tools reached agents by their scope, targets and
//! access rules, opens with every local profile version and draft choosing exactly what reached its agents, and the
//! published versions as immutable as before.
//! `cargo test -p orchester-store --test profile_given_migration profile_given_migration`.

mod support;

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
};

use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, CommandAction, CommandProgram,
    DeniedBehavior, Draft, InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText, ProviderSettings,
    RepositoryAccess, ResourceBody, Skill, Slug, Step, StepKey, StepKind, Tool, ToolAction,
  },
  store::ResourceStore,
};
use orchester_store::{ProjectDb, ProjectMigration};
use rusqlite::Connection;
use support::{at, copied_at_version, model, name};
use tempfile::TempDir;

fn project() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("project");
  (repo, db)
}

/// `body` created as the first version of a resource: a pipeline holds no draft (D-032), so none is created as one.
fn published<B: ResourceBody + Clone>(db: &mut ProjectDb, slug: &str, title: &str, body: &B) -> ResourceId {
  db.create_published(Slug::new(slug).expect("slug"), name(title), body, at(1))
    .expect("create")
    .0
    .id()
}

fn profile(role: &str) -> AgentProfile {
  AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("Work.").expect("prompt"),
    ConnectionId::generate(),
    model(),
    ProviderSettings::default(),
  )
}

fn instruction(targets: AgentTargets, scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new("Follow the rule.").expect("prompt"),
    targets,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("instruction")
}

fn skill(scope: InstructionScope) -> Skill {
  Skill::new("When to use it.".into(), "How.".into(), Vec::new(), scope).expect("skill")
}

fn tool(callers: BTreeMap<ResourceId, AllowedSteps>) -> Tool {
  let access = AccessPolicy::new(
    callers,
    name("Staging"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy");
  let action = CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new());
  Tool::new("", ToolAction::Command(action), Some(access)).expect("tool")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

fn agent_step(step: &str, profile: ResourceId) -> Step {
  Step {
    key: key(step),
    name: name(step),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// The ids of the project's profiles and of what reached each before profiles chose.
struct Seeded {
  builder: ResourceId,
  reviewer: ResourceId,
  writer: ResourceId,
  everyone: ResourceId,
  reviewers_rule: ResourceId,
  writers_rule: ResourceId,
  builder_role: ResourceId,
  pipeline_rule: ResourceId,
  review_rule: ResourceId,
  project_skill: ResourceId,
  build_skill: ResourceId,
  review_tool: ResourceId,
}

/// One instruction for each scope and targets: everyone, the Reviewer role, Writer by id, Builder's role, the pipeline,
/// its `review` step, and one archived; their ids in that order but the archived one's.
fn rules(db: &mut ProjectDb, pipeline: ResourceId, writer: ResourceId) -> [ResourceId; 6] {
  let mut rule = |slug: &str, title: &str, targets, scope| published(db, slug, title, &instruction(targets, scope));
  let all = AgentTargets::All;
  let ids = [
    rule("everyone", "A everyone", all.clone(), InstructionScope::Project),
    rule(
      "reviewers",
      "B reviewers",
      AgentTargets::Roles(BTreeSet::from([name("Reviewer")])),
      InstructionScope::Project,
    ),
    rule(
      "writers",
      "C writers",
      AgentTargets::Profiles(BTreeSet::from([writer])),
      InstructionScope::Project,
    ),
    rule(
      "builder-role",
      "D builder role",
      all.clone(),
      InstructionScope::AgentRole { role: name("Builder") },
    ),
    rule(
      "pipeline",
      "E pipeline",
      all.clone(),
      InstructionScope::Pipeline { pipeline },
    ),
    rule(
      "review",
      "F review",
      all.clone(),
      InstructionScope::Step {
        pipeline,
        step: key("review"),
      },
    ),
  ];
  let archived = rule("archived", "G archived", all, InstructionScope::Project);
  assert!(db.archive_local_resource(archived, at(3)).expect("archive"));
  ids
}

/// Builder and Reviewer run the steps `build` and `review` of a pipeline; Writer runs none. A draft of Writer waits.
fn seeded(db: &mut ProjectDb) -> Seeded {
  let builder = published(db, "builder", "Builder", &profile("Builder"));
  let reviewer = published(db, "reviewer", "Reviewer", &profile("Reviewer"));
  let writer = published(db, "writer", "Writer", &profile("Writer"));
  let pipeline = published(
    db,
    "delivery",
    "Delivery",
    &Pipeline::new(vec![agent_step("build", builder), agent_step("review", reviewer)]).expect("pipeline"),
  );
  let [
    everyone,
    reviewers_rule,
    writers_rule,
    builder_role,
    pipeline_rule,
    review_rule,
  ] = rules(db, pipeline, writer);
  let project_skill = published(db, "project-skill", "Project skill", &skill(InstructionScope::Project));
  let build_skill = published(
    db,
    "build-skill",
    "Build skill",
    &skill(InstructionScope::Step {
      pipeline,
      step: key("build"),
    }),
  );
  let review_tool = published(
    db,
    "review-tool",
    "Review tool",
    &tool(BTreeMap::from([(
      pipeline,
      AllowedSteps::Only(BTreeSet::from([key("review")])),
    )])),
  );
  published(db, "unused-tool", "Unused tool", &tool(BTreeMap::new()));
  db.save_draft(writer, &Draft::new(profile("Writer"), None, at(4)))
    .expect("draft");
  Seeded {
    builder,
    reviewer,
    writer,
    everyone,
    reviewers_rule,
    writers_rule,
    builder_role,
    pipeline_rule,
    review_rule,
    project_skill,
    build_skill,
    review_tool,
  }
}

/// The project's database rewound to before the migration, opened again so it applies.
fn migrated(repo: &TempDir) -> (TempDir, ProjectDb) {
  let old = copied_at_version(
    &repo.path().join(".orchester").join("orchester.db"),
    ProjectDb::version_before(ProjectMigration::ProfileGiven),
  );
  let db = ProjectDb::open(old.path()).expect("migrated");
  (old, db)
}

fn given(profile: &AgentProfile) -> [Vec<ResourceId>; 3] {
  [
    profile.instruction_resources().ids().to_vec(),
    profile.skills().ids().to_vec(),
    profile.tools().ids().to_vec(),
  ]
}

#[test]
fn profile_given_migration_lists_what_reached_each_profile_before() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);

  let (_old, db) = migrated(&repo);

  let latest = |profile| {
    db.latest_version::<AgentProfile>(profile)
      .expect("read")
      .expect("published")
      .body()
      .clone()
  };
  assert_eq!(
    given(&latest(seeded.builder)),
    [
      vec![seeded.everyone, seeded.builder_role, seeded.pipeline_rule],
      vec![seeded.build_skill, seeded.project_skill],
      Vec::new(),
    ],
  );
  assert_eq!(
    given(&latest(seeded.reviewer)),
    [
      vec![
        seeded.everyone,
        seeded.reviewers_rule,
        seeded.pipeline_rule,
        seeded.review_rule
      ],
      vec![seeded.project_skill],
      vec![seeded.review_tool],
    ],
  );
  let writer = [
    vec![seeded.everyone, seeded.writers_rule],
    vec![seeded.project_skill],
    Vec::new(),
  ];
  assert_eq!(given(&latest(seeded.writer)), writer);
  let draft = db.draft::<AgentProfile>(seeded.writer).expect("read").expect("a draft");
  assert_eq!(given(draft.body()), writer);
}

#[test]
fn profile_given_migration_keeps_published_versions_immutable_and_can_run_again() {
  let (repo, mut db) = project();
  let seeded = seeded(&mut db);
  drop(db);
  let (old, db) = migrated(&repo);
  let before = given(
    db.latest_version::<AgentProfile>(seeded.builder)
      .expect("read")
      .expect("published")
      .body(),
  );
  drop(db);

  let file = old.path().join(".orchester").join("orchester.db");
  let raw = Connection::open(&file).expect("the migrated database");
  let changed = raw.execute(
    "UPDATE resource_versions SET body = '{}' WHERE resource_id = ?1",
    [seeded.builder.to_string()],
  );
  assert!(changed.is_err(), "a published version is still immutable");
  // The migration names the outbox a later migration dropped (`team_outbox`); applied again here, it finds it empty.
  raw
    .execute_batch("CREATE TABLE shared_publishes (version_id TEXT)")
    .expect("the outbox as the migration knew it");
  ProjectDb::apply_migration_for_test(&raw, ProjectMigration::ProfileGiven).expect("the migration again");
  drop(raw);

  let db = ProjectDb::open(old.path()).expect("open");
  let again = given(
    db.latest_version::<AgentProfile>(seeded.builder)
      .expect("read")
      .expect("published")
      .body(),
  );
  assert_eq!(again, before);
}
