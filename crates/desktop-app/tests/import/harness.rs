//! Import resources in the real headless window: Kontur (KON), and Personal website (PW) to import from, whose
//! Preview release pipeline (published v1) builds with Builder (published v1, its connection Codex) and has Release
//! rules for it, beside a Reviewer draft nothing uses. Kontur has instructions of its own called Release rules, so the
//! copy of Preview release's needs another name.

use std::{collections::BTreeSet, path::PathBuf};

use orchester_app::routes::{ImportTab, ProjectPage, Route};
use orchester_core::{
  connection::{AgentEndpoint, AgentProvider, Connection, ConnectionKind},
  id::{ConnectionId, ProjectId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, AgentTargets, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, ResourceKind, Slug, Step, StepKey, StepKind,
  },
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::support::{Data, Window, add_project, project_id};

/// Personal website's id.
const SOURCE: &str = "5a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn name(value: &str) -> Name {
  Name::new(value).expect("a name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// The source's resources, by what the tests name them.
pub struct Seeded {
  pub preview: ResourceId,
  pub builder: ResourceId,
  pub reviewer: ResourceId,
  pub rules: ResourceId,
  pub codex: ConnectionId,
}

pub struct Import {
  pub window: Window,
  pub target: PathBuf,
  pub source: Option<(PathBuf, Seeded)>,
}

impl Import {
  /// Kontur and Personal website, at Kontur's Overview.
  pub fn new() -> Self {
    Self::with_source(true)
  }

  /// Kontur alone: there is no other project to import from.
  pub fn alone() -> Self {
    Self::with_source(false)
  }

  fn with_source(source: bool) -> Self {
    let data = Data::new();
    let launch = data.launch();
    let workspace = launch.workspace().expect("the session started");
    let kontur = data.add_kontur(workspace);
    kontur_rules(&kontur.repo_root);
    let source = source.then(|| {
      let repo = data.repo("repo-personal-website");
      let id: ProjectId = SOURCE.parse().expect("a project id");
      add_project(workspace, (id, "PW", "Personal website"), &repo);
      let seeded = personal_website(&repo);
      (repo, seeded)
    });
    Self {
      window: Window::of(data, launch),
      target: kontur.repo_root,
      source,
    }
  }

  pub fn seeded(&self) -> &Seeded {
    &self.source.as_ref().expect("Personal website is there").1
  }

  /// Opens Import resources on `tab` and waits for what it shows first.
  pub fn open(&mut self, tab: ImportTab) {
    self
      .window
      .navigation()
      .go(Route::Project(project_id(), ProjectPage::Import(tab)));
    self.until("the import page", |import| import.exists("screen-import_resources"));
  }

  pub fn route(&self) -> Route {
    self.window.navigation().route_now()
  }

  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} exists"))
      .click();
    self.window.pass();
  }

  pub fn set(&mut self, id: &str, value: &str) {
    self.window.set(id, value);
  }

  /// Chooses the option labelled `label` of the select `id`.
  pub fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    let index = select
      .labels()
      .iter()
      .position(|value| value.as_ref() == label)
      .unwrap_or_else(|| panic!("{id} offers {label}: {:?}", select.labels()));
    select.commit(index);
    self.window.pass();
  }

  /// The labels the select `id` offers.
  pub fn options(&mut self, id: &str) -> Vec<String> {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"))
      .labels()
      .iter()
      .map(ToString::to_string)
      .collect()
  }

  pub fn text(&mut self, id: &str) -> String {
    self.window.text(id)
  }

  /// Whether the element `id` shows.
  pub fn shows(&mut self, id: &str) -> bool {
    self.exists(id)
  }

  /// How many resources of `kind` Kontur has.
  pub fn kontur_count(&self, kind: ResourceKind) -> usize {
    ProjectDb::open(&self.target)
      .expect("Kontur's database")
      .resources(kind)
      .expect("listed")
      .len()
  }

  /// Kontur's resource of `kind` called `wanted`, if there is one.
  pub fn kontur_resource(&self, kind: ResourceKind, wanted: &str) -> Option<ResourceId> {
    ProjectDb::open(&self.target)
      .expect("Kontur's database")
      .resources(kind)
      .expect("listed")
      .into_iter()
      .find(|resource| resource.name().as_str() == wanted)
      .map(|resource| resource.id())
  }

  /// Adds to Kontur, as another window would, an agent profile `title` with the slug `slug`.
  pub fn kontur_profile(&self, slug: &str, title: &str) -> ResourceId {
    ProjectDb::open(&self.target)
      .expect("Kontur's database")
      .create_resource(
        Slug::new(slug).expect("slug"),
        name(title),
        &profile(title, ConnectionId::generate()),
        at(9),
      )
      .expect("Kontur's profile")
      .id()
  }

  /// Sets the schema version of Kontur's database, as another Orchester would, and returns the one it had.
  pub fn kontur_schema(&self, version: u32) -> u32 {
    let db =
      rusqlite::Connection::open(self.target.join(".orchester").join("orchester.db")).expect("Kontur's database");
    let before: u32 = db
      .pragma_query_value(None, "user_version", |row| row.get(0))
      .expect("its schema version");
    db.pragma_update(None, "user_version", version)
      .expect("schema version set");
    before
  }

  /// Adds to Personal website a pipeline Broken release whose two steps are keyed by the ids of Builder and of
  /// `other`: a valid pipeline there, whose copy has one key twice once Builder is `other` in Kontur.
  pub fn colliding_pipeline(&self, other: ResourceId) -> ResourceId {
    let (repo, seeded) = self.source.as_ref().expect("Personal website is there");
    let step = |key: ResourceId| Step {
      key: StepKey::new(&key.to_string()).expect("an id is a valid key"),
      name: name("Build"),
      kind: StepKind::Agent {
        profile: Some(seeded.builder),
        repository: RepositoryAccess::Write,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("prompt"),
      depends_on: BTreeSet::new(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    };
    let pipeline = Pipeline::new(vec![step(seeded.builder), step(other)]).expect("distinct keys in Personal website");
    ProjectDb::open(repo)
      .expect("Personal website's database")
      .create_resource(
        Slug::new("broken-release").expect("slug"),
        name("Broken release"),
        &pipeline,
        at(10),
      )
      .expect("Broken release")
      .id()
  }

  /// Goes from Select resources to Create drafts with Preview release selected, Release rules renamed.
  pub fn go_to_create(&mut self) {
    let seeded = (self.seeded().preview, self.seeded().rules);
    self.until("the list", |import| {
      import.exists(&format!("import-check-{}", seeded.0))
    });
    self.click(&format!("import-check-{}", seeded.0));
    self.click("import-next");
    self.until("Resolve dependencies", |import| import.exists("import-mappings"));
    self.choose(&format!("import-choice-{}", seeded.1), "Rename copy");
    self.set(
      &format!("import-rename-{}", seeded.1),
      "Release rules (Personal website)",
    );
    self.click("import-continue");
    self.until("Create drafts", |import| import.exists("import-ready-head"));
  }
}

/// Kontur's own Release rules.
fn kontur_rules(repo: &std::path::Path) {
  let mut db = ProjectDb::open(repo).expect("Kontur's database");
  db.create_resource(
    Slug::new("our-release-rules").expect("slug"),
    name("Release rules"),
    &instruction(InstructionScope::Project),
    at(1),
  )
  .expect("Kontur's rules");
}

fn instruction(scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new("Keep the changelog.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("an instruction")
}

fn profile(role: &str, connection: ConnectionId) -> AgentProfile {
  AgentProfile::new(
    name(role),
    String::new(),
    PromptText::new("Build it.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
}

fn personal_website(repo: &std::path::Path) -> Seeded {
  let mut db = ProjectDb::open(repo).expect("Personal website's database");
  let codex = Connection::new(
    ConnectionId::generate(),
    name("Codex · Personal website"),
    ConnectionKind::Agent(AgentProvider::CodexCli),
    AgentEndpoint::Program(std::env::temp_dir().join("codex")),
    at(1),
  )
  .expect("a connection");
  db.create_connection(&codex).expect("stored");
  let create = |slug: &str, title: &str| (Slug::new(slug).expect("slug"), name(title));
  let (slug, title) = create("builder", "Builder");
  let builder = db
    .create_resource(slug, title, &profile("Builder", codex.id()), at(2))
    .expect("Builder")
    .id();
  db.publish_draft::<AgentProfile>(builder, at(3)).expect("published");
  let (slug, title) = create("reviewer", "Reviewer");
  let reviewer = db
    .create_resource(slug, title, &profile("Reviewer", codex.id()), at(4))
    .expect("Reviewer")
    .id();
  let build = Step {
    key: StepKey::new("build").expect("key"),
    name: name("Build"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let (slug, title) = create("preview-release", "Preview release");
  let preview = db
    .create_resource(slug, title, &Pipeline::new(vec![build]).expect("a pipeline"), at(5))
    .expect("Preview release")
    .id();
  db.publish_draft::<Pipeline>(preview, at(6)).expect("published");
  let (slug, title) = create("release-rules", "Release rules");
  let rules = db
    .create_resource(
      slug,
      title,
      &instruction(InstructionScope::Pipeline { pipeline: preview }),
      at(7),
    )
    .expect("Release rules")
    .id();
  Seeded {
    preview,
    builder,
    reviewer,
    rules,
    codex: codex.id(),
  }
}
