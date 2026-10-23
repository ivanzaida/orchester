//! Import resources (`Dzzks`): Kontur with the design's catalogue, its own Codex connection, a draft tool called
//! Deploy preview and its secrets; and Personal website (PW), registered beside it, to import from. Personal
//! website's Preview release v3 implements with Builder v2, builds with Build preview and deploys with Deploy preview
//! v2, which only Preview release may call; Content checks reviews with Reviewer and runs Link checker, with Release
//! notes style written for it; Implementation standards is a project rule; Preview host is an HTTP tool whose token is
//! the secret PREVIEW_HOST_API_KEY.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ProjectId, ResourceId, SecretId},
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, CommandAction, CommandArg,
    CommandProgram, DeniedBehavior, Draft, HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction,
    InstructionScope, ModelId, Pipeline, PipelineEdit, PromptText, ProviderSettings, RepositoryAccess, ResourceBody,
    Slug, StepKind, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ResourceStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};

use crate::kontur::{Kontur, SeedError, catalogue, files, git, name};

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  kontur.catalogue()?;
  let now = kontur.ago(60);
  let programs = kontur.repo.join(".orchester").join("recorded");
  catalogue::recorded_connection(
    &mut kontur.db,
    ("Codex · Kontur", AgentProvider::CodexCli, &programs.join("codex")),
    now,
  )?;
  let deploy = command("npm", &["run", "deploy"]);
  kontur.db.create_resource(
    Slug::new("deploy-preview").expect("slug"),
    name("Deploy preview"),
    &deploy,
    now,
  )?;
  for (title, environment) in [
    ("STAGING_DEPLOY_TOKEN", Environment::Staging),
    ("DOCS_API_KEY", Environment::Development),
  ] {
    kontur.db.create_secret(&secret(title, environment, now))?;
  }
  personal_website(kontur)
}

/// Personal website, a repository of its own beside Kontur's, registered in the app's database before Kontur.
fn personal_website(kontur: &Kontur) -> Result<(), SeedError> {
  let folder = kontur.app_database.with_file_name("repo-personal-website");
  std::fs::create_dir_all(&folder).map_err(|source| SeedError::Io {
    path: folder.clone(),
    source,
  })?;
  git::init(&folder, &files::initial())?;
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("PW").expect("a valid project key"),
    name("Personal website"),
  );
  let mut db = ProjectDb::create(&folder, project.clone())?;
  let now = kontur.ago(2 * 24 * 60);
  website_catalogue(&mut db, &folder, now)?;
  drop(db);
  let mut apps = AppDb::open(&kontur.app_database)?;
  // The signed-in account's project, or the next one's to sign in.
  let owner = apps.cloud_account()?.map(|account| account.account);
  // Opened a day before Kontur, which stays the project the app opens.
  apps.register_project(&project, &folder, kontur.ago(24 * 60), owner.as_ref())?;
  Ok(())
}

fn website_catalogue(db: &mut ProjectDb, folder: &std::path::Path, now: Timestamp) -> Result<(), SeedError> {
  let codex = catalogue::recorded_connection(
    db,
    (
      "Codex · Personal website",
      AgentProvider::CodexCli,
      &folder.join(".orchester").join("recorded").join("codex"),
    ),
    now,
  )?;
  let builder = publish(
    db,
    "Builder",
    vec![
      profile("Builder", codex, "Builds the site."),
      profile("Builder", codex, "Builds the site in small commits."),
    ],
    now,
  )?;
  let reviewer = publish(
    db,
    "Reviewer",
    vec![profile("Reviewer", codex, "Reviews the change.")],
    now,
  )?;
  let build = publish(db, "Build preview", vec![command("npm", &["run", "build"])], now)?;
  let links = publish(db, "Link checker", vec![command("npx", &["linkinator", "dist"])], now)?;
  let deploy = publish(db, "Deploy preview", vec![command("npm", &["run", "deploy"])], now)?;
  let preview = pipeline(&[
    ("Implement", Executor::Agent(builder)),
    ("Build", Executor::Tool(build)),
    ("Deploy", Executor::Tool(deploy)),
  ]);
  let versions = ["First release.", "Builds before it deploys.", "Deploys a preview."]
    .map(|description| preview.clone().with_description(description.to_owned()));
  let preview = publish(db, "Preview release", versions.to_vec(), now)?;
  let checks = pipeline(&[
    ("Review", Executor::Agent(reviewer)),
    ("Check links", Executor::Tool(links)),
  ]);
  let checks = publish(db, "Content checks", vec![checks], now)?;
  // Deploy preview v2: only Preview release may call it.
  let guarded = Tool::new(
    "",
    ToolAction::Command(command_action("npm", &["run", "deploy"])),
    Some(access(BTreeMap::from([(preview, AllowedSteps::All)]))?),
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  republish(db, deploy, guarded, now)?;
  let standards = ["Keep each change small.", "Keep each change small and tested."]
    .map(|text| instruction(text, InstructionScope::Project));
  publish(db, "Implementation standards", standards.to_vec(), now)?;
  let notes = instruction(
    "Write release notes for people.",
    InstructionScope::Pipeline { pipeline: checks },
  );
  publish(db, "Release notes style", vec![notes], now)?;
  let key = secret("PREVIEW_HOST_API_KEY", Environment::Staging, now);
  db.create_secret(&key)?;
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://preview.example.test/deploy".into(),
    "preview.example.test".into(),
    HttpAuth::Bearer { secret: key.id() },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))?;
  // No pipeline calls Preview host yet: its policy denies every caller.
  let host = Tool::new("", ToolAction::Http(request), Some(access(BTreeMap::new())?))
    .map_err(|error| SeedError::Invalid(error.to_string()))?;
  db.create_resource(
    Slug::new("preview-host").expect("slug"),
    name("Preview host"),
    &host,
    now,
  )?;
  Ok(())
}

/// A staging policy that lets `callers` call the tool, after asking every call.
fn access(callers: BTreeMap<ResourceId, AllowedSteps>) -> Result<AccessPolicy, SeedError> {
  AccessPolicy::new(
    callers,
    name("Staging"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .map_err(|error| SeedError::Invalid(error.to_string()))
}

/// What a step runs.
enum Executor {
  Agent(ResourceId),
  Tool(ResourceId),
}

/// A pipeline of `steps`, each after the one before it.
fn pipeline(steps: &[(&str, Executor)]) -> Pipeline {
  let mut edit = PipelineEdit::new(Pipeline::new(vec![]).expect("an empty draft"));
  let mut before = None;
  for (title, executor) in steps {
    let key = edit.add_step(name(title), before.as_ref());
    let kind = match executor {
      Executor::Agent(profile) => StepKind::Agent {
        profile: Some(*profile),
        repository: RepositoryAccess::Write,
        model: None,
        permissions: Default::default(),
      },
      Executor::Tool(tool) => StepKind::Tool { tool: Some(*tool) },
    };
    edit.update(&key, |step| step.kind = kind);
    before = Some(key);
  }
  edit.validate().expect("every step runs something")
}

/// Creates `title`'s resource with the first of `bodies` and publishes each in turn: version N is the Nth.
fn publish<B: ResourceBody>(
  db: &mut ProjectDb,
  title: &str,
  bodies: Vec<B>,
  now: Timestamp,
) -> Result<ResourceId, SeedError> {
  let mut bodies = bodies.into_iter();
  let first = bodies.next().expect("every resource has a first version");
  let slug = Slug::from_name(title).expect("the seed's names have letters");
  let id = db.create_resource(slug, name(title), &first, now)?.id();
  db.publish_draft::<B>(id, now)?;
  for body in bodies {
    republish(db, id, body, now)?;
  }
  Ok(id)
}

/// Publishes `body` as `id`'s next version.
fn republish<B: ResourceBody>(db: &mut ProjectDb, id: ResourceId, body: B, now: Timestamp) -> Result<(), SeedError> {
  let latest = db.latest_version::<B>(id)?.map(|version| version.number());
  db.save_draft(id, &Draft::new(body, latest, now))?;
  db.publish_draft::<B>(id, now)?;
  Ok(())
}

fn profile(role: &str, connection: ConnectionId, duties: &str) -> AgentProfile {
  AgentProfile::new(
    name(role),
    duties.to_owned(),
    PromptText::new(duties).expect("a valid prompt"),
    connection,
    ModelId::new(catalogue::MODEL).expect("a valid model id"),
    ProviderSettings::default(),
  )
}

fn command_action(program: &str, args: &[&str]) -> CommandAction {
  CommandAction::new(
    CommandProgram::new(program).expect("a valid program"),
    args
      .iter()
      .map(|arg| CommandArg::new(arg).expect("a valid argument"))
      .collect(),
  )
}

fn command(program: &str, args: &[&str]) -> Tool {
  Tool::command("", command_action(program, args))
}

fn instruction(text: &str, scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("a valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("a valid instruction")
}

fn secret(title: &str, environment: Environment, now: Timestamp) -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(title).expect("a valid secret name"),
      kind: SecretKind::ApiKey,
      environment,
    },
    now,
  )
}
