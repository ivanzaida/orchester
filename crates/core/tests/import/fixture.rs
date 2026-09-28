//! Two projects as an import sees them: "Personal website", whose "Preview release" pipeline runs
//! the Builder profile and calls the Deploy preview tool, and "Kontur", the project it is
//! imported into.

use std::{
  collections::{BTreeMap, BTreeSet},
  num::NonZeroU32,
};

use orchester_core::{
  connection::{AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ProjectId, ResourceId, SecretId},
  import::{Catalog, CatalogResource, Content},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, DeniedBehavior, HttpAuth,
    HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Resource, Slug, Step, StepKey, StepKind, Tool, ToolAction, VersionNumber,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  time::Timestamp,
};

pub fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub fn version(number: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(number).expect("versions start at 1"))
}

fn project(title: &str, key: &str) -> Project {
  Project::new(ProjectId::generate(), ProjectKey::new(key).expect("key"), name(title))
}

/// The resource `id` with its `published` version and `draft`.
fn entry(
  id: ResourceId,
  slug: &str,
  title: &str,
  published: Option<(u32, Content)>,
  draft: Option<Content>,
) -> CatalogResource {
  let kind = published.as_ref().map_or_else(
    || draft.as_ref().map(Content::kind),
    |(_, content)| Some(content.kind()),
  );
  let resource = Resource::new(
    id,
    kind.expect("some content"),
    Slug::new(slug).expect("slug"),
    name(title),
    at(1),
  );
  let published = published.map(|(number, content)| (version(number), content));
  CatalogResource::new(resource, published, draft).expect("content")
}

/// A resource with only a published version.
pub fn published(slug: &str, title: &str, number: u32, content: Content) -> CatalogResource {
  entry(ResourceId::generate(), slug, title, Some((number, content)), None)
}

/// A resource never published.
pub fn draft(slug: &str, title: &str, content: Content) -> CatalogResource {
  entry(ResourceId::generate(), slug, title, None, Some(content))
}

pub fn codex(title: &str) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    name(title),
    ConnectionKind::Agent(AgentProvider::CodexCli),
    std::env::temp_dir().join("codex"),
    at(1),
  )
  .expect("absolute executable")
}

pub fn claude(title: &str) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    name(title),
    ConnectionKind::Agent(AgentProvider::ClaudeCodeCli),
    std::env::temp_dir().join("claude"),
    at(1),
  )
  .expect("absolute executable")
}

pub fn secret(title: &str) -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(title).expect("secret name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Staging,
    },
    at(1),
  )
}

pub fn profile(role: &str, connection: ConnectionId) -> Content {
  Content::AgentProfile(AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("Build it.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  ))
}

fn step(key: &str, kind: StepKind) -> Step {
  Step {
    key: StepKey::new(key).expect("key"),
    name: name(key),
    kind,
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

/// Build with `profile`, then deploy with `tool`.
pub fn pipeline(profile: ResourceId, tool: ResourceId, description: &str) -> Content {
  let build = step(
    "build",
    StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
  );
  let deploy = Step {
    depends_on: BTreeSet::from([build.key.clone()]),
    ..step("deploy", StepKind::Tool { tool: Some(tool) })
  };
  Content::Pipeline(
    Pipeline::new(vec![build, deploy])
      .expect("valid pipeline")
      .with_description(description.to_owned()),
  )
}

/// An HTTP tool whose bearer token is `secret`, callable by every step of `callers`.
pub fn http_tool(secret: SecretId, callers: &[ResourceId]) -> Content {
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://preview.example.test/deploy".into(),
    "preview.example.test".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("valid request");
  let access = AccessPolicy::new(
    callers.iter().map(|caller| (*caller, AllowedSteps::All)).collect(),
    name("Staging"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("valid policy");
  Content::Tool(Tool::new("", ToolAction::Http(request), Some(access)).expect("valid tool"))
}

pub fn instruction(text: &str, targets: AgentTargets, scope: InstructionScope) -> Content {
  Content::Instruction(
    Instruction::new(
      PromptText::new(text).expect("prompt"),
      targets,
      BTreeSet::from([InjectionEvent::Start]),
      scope,
    )
    .expect("valid instruction"),
  )
}

/// Personal website: Preview release v3 (and a draft) builds with Builder v2 and deploys with
/// Deploy preview v2, whose token is PREVIEW_HOST_API_KEY and which Preview release and Nightly
/// may call; Release rules are Preview release's instructions; Reviewer is a draft.
pub struct Source {
  pub catalog: Catalog,
  pub codex: ConnectionId,
  pub key: SecretId,
  pub builder: ResourceId,
  pub reviewer: ResourceId,
  pub deploy: ResourceId,
  pub pipeline: ResourceId,
  pub nightly: ResourceId,
  pub rules: ResourceId,
}

pub fn source() -> Source {
  let codex = codex("Codex");
  let key = secret("PREVIEW_HOST_API_KEY");
  let builder = published("builder", "Builder", 2, profile("Builder", codex.id()));
  let reviewer = draft("reviewer", "Reviewer", profile("Reviewer", codex.id()));
  let nightly_id = ResourceId::generate();
  let pipeline_id = ResourceId::generate();
  let deploy = published(
    "deploy-preview",
    "Deploy preview",
    2,
    http_tool(key.id(), &[pipeline_id, nightly_id]),
  );
  let (builder_id, deploy_id) = (builder.resource().id(), deploy.resource().id());
  let preview = entry(
    pipeline_id,
    "preview-release",
    "Preview release",
    Some((3, pipeline(builder_id, deploy_id, "Published"))),
    Some(pipeline(builder_id, deploy_id, "Draft")),
  );
  let nightly = entry(
    nightly_id,
    "nightly",
    "Nightly",
    None,
    Some(pipeline(builder_id, deploy_id, "")),
  );
  let rules = published(
    "release-rules",
    "Release rules",
    1,
    instruction(
      "Keep the changelog.",
      AgentTargets::All,
      InstructionScope::Pipeline { pipeline: pipeline_id },
    ),
  );
  let ids = [builder_id, reviewer.resource().id(), deploy_id, rules.resource().id()];
  Source {
    catalog: Catalog::new(
      project("Personal website", "PW"),
      vec![preview, nightly, builder, reviewer, rules, deploy],
      vec![codex.clone()],
      Vec::new(),
      vec![key.clone()],
    ),
    codex: codex.id(),
    key: key.id(),
    builder: ids[0],
    reviewer: ids[1],
    deploy: ids[2],
    pipeline: pipeline_id,
    nightly: nightly_id,
    rules: ids[3],
  }
}

/// Kontur with `resources`, a Codex and a Claude Code connection and a secret of its own.
pub struct Target {
  pub catalog: Catalog,
  pub codex: ConnectionId,
  pub claude: ConnectionId,
  pub key: SecretId,
}

pub fn target(resources: Vec<CatalogResource>) -> Target {
  let (codex, claude, key) = (codex("Codex"), claude("Claude"), secret("PREVIEW_HOST_API_KEY"));
  Target {
    catalog: Catalog::new(
      project("Kontur", "KON"),
      resources,
      vec![codex.clone(), claude.clone()],
      Vec::new(),
      vec![key.clone()],
    ),
    codex: codex.id(),
    claude: claude.id(),
    key: key.id(),
  }
}
