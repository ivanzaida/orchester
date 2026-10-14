//! Kontur in a git repository of its own, with a Claude Code connection to `orchester-fake-claude` (and a Codex one
//! the Orchestrator cannot run on), its runner the real one built with the fake CLIs, and the window on its
//! Orchestrator.

use std::{
  collections::{BTreeMap, BTreeSet},
  env, fs,
  path::{Path, PathBuf},
  process::Command,
  sync::OnceLock,
  time::{Duration, Instant},
};

use orchester_app::{
  routes::{OrchestratorChat, ProjectPage, Route},
  startup::Launch,
};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  orchestrator::OrchestratorSettings,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::Project,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::app_paths::runner_beside;

use super::support::{Data, Window, add_project};

/// The runner, the fake Claude Code and the fake Codex, built into this profile's folder once per test process.
fn programs() -> (PathBuf, PathBuf, PathBuf) {
  static BUILT: OnceLock<(PathBuf, PathBuf, PathBuf)> = OnceLock::new();
  BUILT.get_or_init(build_programs).clone()
}

fn build_programs() -> (PathBuf, PathBuf, PathBuf) {
  let status = Command::new(env!("CARGO"))
    .args([
      "build",
      "--quiet",
      "-p",
      "orchester-runner",
      "--features",
      "test-agent",
      "--bins",
    ])
    .status()
    .expect("run cargo");
  assert!(status.success(), "cargo build -p orchester-runner: {status}");
  let test = env::current_exe().expect("the test's path");
  let profile = test
    .parent()
    .and_then(Path::parent)
    .expect("tests live in <target>/<profile>/deps");
  let fake = |name: &str| profile.join(name).with_extension(env::consts::EXE_EXTENSION);
  (
    runner_beside(&profile.join("orchester-app")),
    fake("orchester-fake-claude"),
    fake("orchester-fake-codex"),
  )
}

fn git(repo: &Path, args: &[&str]) {
  let output = Command::new("git")
    .arg("-C")
    .arg(repo)
    .args(args)
    .output()
    .expect("run git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// How the test's session starts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Setup {
  /// The Orchestrator runs on the Claude Code connection, on the built runner.
  Ready,
  /// The Orchestrator runs on the Codex connection, on the built runner.
  Codex,
  /// Nothing was chosen for the Orchestrator yet.
  NotChosen,
  /// Chosen, but the project's runner cannot start: its program does not exist.
  NoRunner,
}

pub struct Orchestrator {
  /// A project id of the test's own: the runner keeps its private files by project, so tests running at once must
  /// not share one.
  pub project: ProjectId,
  pub window: Window,
  pub root: PathBuf,
  pub claude: ConnectionId,
}

impl Orchestrator {
  pub fn new(setup: Setup) -> Self {
    let (runner, fake, fake_codex) = programs();
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orchestrator-tests");
    fs::create_dir_all(&scratch).expect("scratch directory");
    let data = Data {
      dir: tempfile::tempdir_in(scratch).expect("test directory"),
      runtime: tokio::runtime::Runtime::new().expect("runtime"),
    };
    let repo = data.dir.path().join("kontur");
    fs::create_dir_all(&repo).expect("the repository's folder");
    git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    git(&repo, &["config", "user.name", "Orchester Test"]);
    git(&repo, &["config", "user.email", "test@orchester.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("README.md"), "hi\n").expect("the readme");
    git(&repo, &["add", "README.md"]);
    git(&repo, &["commit", "--quiet", "-m", "initial"]);
    let runner = match setup {
      Setup::NoRunner => data.dir.path().join("no-runner").join("orchester-runner"),
      Setup::Ready | Setup::Codex | Setup::NotChosen => runner,
    };
    let launch = Launch::begin_at(&data.database(), runner, data.runtime.handle(), None);
    let project = ProjectId::generate();
    let open = add_project(
      launch.workspace().expect("the session started"),
      (project, "KON", "Kontur"),
      &repo,
    );
    let mut db = ProjectDb::open(&open.repo_root).expect("database");
    let claude = connection(AgentProvider::ClaudeCodeCli, "Claude · Kontur", &fake);
    db.create_connection(&claude).expect("Claude Code connection");
    let codex = connection(AgentProvider::CodexCli, "Codex · Kontur", &fake_codex);
    db.create_connection(&codex).expect("Codex connection");
    if setup != Setup::NotChosen {
      let on = if setup == Setup::Codex { codex.id() } else { claude.id() };
      let settings = OrchestratorSettings::new(on, ModelId::new("haiku").expect("model"), None);
      db.publish_orchestrator(&settings, Timestamp::now()).expect("settings");
    }
    Self {
      project,
      window: Window::of(data, launch),
      root: open.repo_root,
      claude: claude.id(),
    }
  }

  pub fn go(&mut self, chat: OrchestratorChat) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Orchestrator(chat)));
    // What it runs on is read: the composer sends from then on.
    self.until("the Orchestrator", |screen| {
      screen.exists("orchestrator-composer") && !screen.text("orchestrator-runs-on").is_empty()
    });
  }

  pub fn settings(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::OrchestratorSettings));
    self.until("its settings", |screen| screen.exists("orchestrator-settings-form"));
  }

  pub fn route(&self) -> Route {
    self.window.navigation().route_now()
  }

  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      if Instant::now() >= deadline {
        let shown = [
          self.text("screen-orchestrator"),
          self.text("screen-orchestrator_settings"),
        ]
        .join(" ");
        let agent = fs::read_to_string(self.root.join(".orchester").join("fake-chat.log")).unwrap_or_default();
        panic!(
          "timed out waiting for {what}; the screen shows: {shown}
the agent logged: {agent}"
        );
      }
      std::thread::sleep(Duration::from_millis(5));
    }
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Clicks the element `id`.
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} exists"))
      .click();
    self.window.pass();
    self.window.pass();
  }

  /// The labels of the select `id`'s options.
  pub fn options(&mut self, id: &str) -> Vec<String> {
    let select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    select.labels().iter().map(ToString::to_string).collect()
  }

  /// Chooses the option `label` of the select `id`.
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
      .position(|option| &**option == label)
      .unwrap_or_else(|| panic!("{id} offers {label}"));
    select.commit(index);
    self.window.pass();
  }

  pub fn set(&mut self, id: &str, text: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(text);
    self.window.pass();
  }

  /// A key pressed where the focus is.
  pub fn key(&mut self, (key, code): (&str, &str), ctrl: bool) {
    self
      .window
      .tree
      .key_down(key.to_owned(), code.to_owned(), false, ctrl, false);
    self
      .window
      .tree
      .key_up(key.to_owned(), code.to_owned(), false, ctrl, false);
    self.window.pass();
  }

  /// Where `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// The texts inside `id`, joined; empty when it is not shown.
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_default().join(" ")
  }

  /// Writes `text` and sends it with Ctrl+Enter, the focus in the field when the key is pressed, once the message
  /// sent before is done with: the field shows its placeholder again.
  pub fn send(&mut self, text: &str) {
    self.until("the composer to be free", |screen| {
      let shown = screen.text("orchestrator-input");
      shown.starts_with("Message Orchestrator") || shown.starts_with("Describe what you want")
    });
    self.set("orchestrator-input", text);
    self.click("orchestrator-input");
    self
      .window
      .tree
      .get_element_by_id_mut("orchestrator-input")
      .expect("the message field")
      .focus();
    self.key(("Enter", "Enter"), true);
  }

  /// The project's database.
  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("database")
  }

  /// Kontur named `name` in the list of projects, as when its database names it differently.
  pub fn rename(&self, name: &str) {
    let key = self.db().project().key().clone();
    AppDb::open(&self.window.data.database())
      .expect("the app's database")
      .register_project(
        &Project::new(self.project, key, named(name)),
        &self.root,
        Timestamp::now(),
      )
      .expect("renamed");
  }

  /// Another project named `name`, in a repository of its own; its folder.
  pub fn another(&self, name: &str) -> PathBuf {
    let repo = self.window.data.repo("another");
    let workspace = self.window.launch.workspace().expect("the session started");
    add_project(workspace, (ProjectId::generate(), "OTH", name), &repo).repo_root
  }

  /// A published Delivery pipeline whose one step's Builder reads on the Claude Code connection.
  pub fn delivery(&self) -> ResourceId {
    let builder = self.builder();
    self.pipeline(builder, ("delivery", "Delivery"))
  }

  /// The published Builder profile, on the Claude Code connection, reading only.
  pub fn builder(&self) -> ResourceId {
    let mut db = self.db();
    let now = Timestamp::now();
    let profile = AgentProfile::new(
      named("Builder"),
      "Implements tasks.".into(),
      PromptText::new("Work carefully.").expect("prompt"),
      self.claude,
      ModelId::new("haiku").expect("model"),
      ProviderSettings::new(BTreeMap::new()).expect("settings"),
    )
    .with_permissions(AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    ));
    let builder = db
      .create_resource(Slug::new("builder").expect("slug"), named("Builder"), &profile, now)
      .expect("profile")
      .id();
    db.publish_draft::<AgentProfile>(builder, now).expect("publish profile");
    builder
  }

  /// A published pipeline `name` (`slug`) of one step by `builder`.
  pub fn pipeline(&self, builder: ResourceId, (slug, name): (&str, &str)) -> ResourceId {
    let mut db = self.db();
    let now = Timestamp::now();
    let step = Step {
      key: StepKey::new("implement").expect("key"),
      name: named("Implement"),
      kind: StepKind::Agent {
        profile: Some(builder),
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("Read README.md.").expect("prompt"),
      depends_on: BTreeSet::new(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    };
    let pipeline = Pipeline::new(vec![step]).expect("pipeline");
    let created = db
      .create_resource(Slug::new(slug).expect("slug"), named(name), &pipeline, now)
      .expect("pipeline")
      .id();
    db.publish_draft::<Pipeline>(created, now).expect("publish pipeline");
    created
  }

  /// The person's *Let automation change agent permissions*.
  pub fn let_automation_widen(&self, allowed: bool) {
    self
      .window
      .launch
      .workspace()
      .expect("the session started")
      .set_automation_changes_permissions(allowed)
      .expect("the person's choice is kept");
  }

  /// A draft task titled `title` on `pipeline`, complete enough to run; its key.
  pub fn draft_on(&self, pipeline: ResourceId, title: &str) -> String {
    let details = TaskDetails {
      title: named(title),
      brief: "Read the readme.".into(),
      acceptance_criteria: vec!["The readme was read.".into()],
      pipeline: Some(pipeline),
      limits: TaskLimits::new(None, None).expect("no limits"),
    };
    self
      .db()
      .create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
      .expect("task")[0]
      .key()
      .to_string()
  }
}

fn named(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// A checked connection of `provider` named `name` to the fake CLI at `program`, listing `haiku` and `opus`.
fn connection(provider: AgentProvider, name: &str, program: &Path) -> AgentConnection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    provider,
    program.to_path_buf(),
    Timestamp::now(),
  )
  .expect("absolute program");
  let model = |id: &str| CatalogModel::new(ModelId::new(id).expect("model"), vec!["low".into(), "high".into()]);
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, vec![model("haiku"), model("opus")]),
      },
    })
    .expect("check");
  connection
}
