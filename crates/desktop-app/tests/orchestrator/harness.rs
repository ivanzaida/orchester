//! Kontur in a git repository of its own, with a Claude Code connection to `orchester-fake-claude` and a Codex one to
//! `orchester-fake-codex`, its runner the real one built with the fake CLIs, and the window on its Orchestrator.

use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::{Duration, Instant},
};

use lurq::app::events::MouseButton;
use orchester_app::routes::{OrchestratorChat, ProjectPage, Route};
use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  orchestrator::OrchestratorSettings,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, StepPermissions},
  project::Project,
  resource::{
    AgentProfile, AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  run::AllowAutopilot,
  store::{ConnectionStore, ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::workspace::OrchestratorConsent;

use super::{
  connections,
  support::{Data, Window, add_project, runner_programs::programs, stand_ins::StandIns},
};

pub fn git(repo: &Path, args: &[&str]) {
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
  /// The project has no connection at all, so nothing is chosen either.
  NoConnection,
  /// Only a native agent connection, which the Orchestrator does not run on: nothing is chosen.
  NativeOnly,
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
  pub codex: ConnectionId,
}

impl Orchestrator {
  pub fn new(setup: Setup) -> Self {
    Self::started(setup, None, false).0
  }

  /// As [`Orchestrator::new`], with the system's pickers and clipboard replaced (`support::stand_ins`).
  pub fn with_stand_ins(setup: Setup) -> (Self, StandIns) {
    let (screen, stand_ins) = Self::started(setup, None, true);
    (screen, stand_ins.expect("the stand-ins"))
  }

  /// [`Orchestrator::new`] whose window runs git (and gh) with `git_programs` when they are given.
  pub fn with_git(setup: Setup, git_programs: Option<Programs>) -> Self {
    Self::started(setup, git_programs, false).0
  }

  fn started(setup: Setup, git_programs: Option<Programs>, stand_ins: bool) -> (Self, Option<StandIns>) {
    let programs = programs();
    let (runner, fake, fake_codex) = (programs.runner.clone(), &programs.fake_claude, &programs.fake_codex);
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orchestrator-tests");
    fs::create_dir_all(&scratch).expect("scratch directory");
    let data = Data::at(
      tempfile::tempdir_in(scratch).expect("test directory"),
      tokio::runtime::Runtime::new().expect("runtime"),
    );
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
      Setup::Ready | Setup::Codex | Setup::NotChosen | Setup::NoConnection | Setup::NativeOnly => runner,
    };
    let launch = data.launch_with(runner);
    let launch = match git_programs {
      Some(git_programs) => launch.with_programs(git_programs),
      None => launch,
    };
    let project = ProjectId::generate();
    let open = add_project(
      launch.workspace().expect("the session started"),
      (project, "KON", "Kontur"),
      &repo,
    );
    let mut db = ProjectDb::open(&open.repo_root).expect("database");
    let (claude, codex) = connect(&mut db, setup, (fake.as_path(), fake_codex.as_path()));
    let (window, stand_ins) = if stand_ins {
      let (window, stand_ins) = Window::launched_with_stand_ins(data, launch);
      (window, Some(stand_ins))
    } else {
      (Window::of(data, launch), None)
    };
    let screen = Self {
      project,
      window,
      root: open.repo_root,
      claude,
      codex,
    };
    (screen, stand_ins)
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

  /// The labels of the select `id`'s options; of a choice field's (`ui::choice_field`), its menu's rows, which it
  /// opens and closes again to read them.
  pub fn options(&mut self, id: &str) -> Vec<String> {
    if let Some(select) = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
    {
      return select.labels().iter().map(ToString::to_string).collect();
    }
    let opened = !self.is_open(id);
    if opened {
      self.click(id);
    }
    let labels = (0..)
      .map(|index| format!("{id}-option-{index}"))
      .map_while(|option| self.window.texts(&option).and_then(|texts| texts.first().cloned()))
      .collect();
    if opened && self.is_open(id) {
      self.click(id);
    }
    labels
  }

  /// Whether the select `id`'s menu is open; a choice field's while its menu is shown.
  pub fn is_open(&mut self, id: &str) -> bool {
    if let Some(select) = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
    {
      return select.is_open();
    }
    self.exists(&format!("{id}-menu"))
  }

  /// Presses the pointer on the middle of `id`, as a person would: whatever takes presses there gets it.
  pub fn press(&mut self, id: &str) {
    let frame = self.bounds(id);
    let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
    let tree = &mut self.window.tree;
    tree.mouse_move(x, y);
    tree.mouse_down(x, y, MouseButton::Left);
    tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
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
      screen
        .text("orchestrator-input")
        .starts_with("Message the Orchestrator")
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
    let mut db = AppDb::open(&self.window.data.database()).expect("the app's database");
    // The project stays the signed-in account's.
    let owner = db.cloud_account().expect("the account").map(|account| account.account);
    db.register_project(
      &Project::new(self.project, key, named(name)),
      &self.root,
      Timestamp::now(),
      owner.as_ref(),
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
    let created = db
      .create_resource(
        Slug::new(slug).expect("slug"),
        named(name),
        &one_step(builder, "Read README.md."),
        now,
      )
      .expect("pipeline")
      .id();
    db.publish_draft::<Pipeline>(created, now).expect("publish pipeline");
    created
  }

  /// Saves a draft of `pipeline`, its one step by `builder` told `instructions`, over its latest version.
  pub fn pipeline_draft(&self, (pipeline, builder): (ResourceId, ResourceId), instructions: &str) {
    let body = one_step(builder, instructions);
    self
      .db()
      .update_draft::<Pipeline>(pipeline, |_, latest| {
        let based_on = latest.map(|version| version.number());
        Ok(Draft::new(body, based_on, Timestamp::now()))
      })
      .expect("draft saved");
  }

  /// A published instruction `name` that applies to step *Implement* of `pipeline`, telling `text`.
  pub fn instruction(&self, pipeline: ResourceId, (name, text): (&str, &str)) -> ResourceId {
    let mut db = self.db();
    let now = Timestamp::now();
    let body = instruction_body(pipeline, text);
    let slug = Slug::new(&name.to_lowercase().replace(' ', "-")).expect("slug");
    let created = db
      .create_resource(slug, named(name), &body, now)
      .expect("instruction")
      .id();
    db.publish_draft::<Instruction>(created, now)
      .expect("publish instruction");
    created
  }

  /// Saves a draft of `instruction` (of `pipeline`'s step) telling `text`, over its latest version.
  pub fn instruction_draft(&self, (instruction, pipeline): (ResourceId, ResourceId), text: &str) {
    let body = instruction_body(pipeline, text);
    self
      .db()
      .update_draft::<Instruction>(instruction, |_, latest| {
        Ok(Draft::new(
          body,
          latest.map(|version| version.number()),
          Timestamp::now(),
        ))
      })
      .expect("draft saved");
  }

  /// The number of `pipeline`'s latest published version.
  pub fn pipeline_version(&self, pipeline: ResourceId) -> u32 {
    self
      .db()
      .latest_version::<Pipeline>(pipeline)
      .expect("versions")
      .expect("a published version")
      .number()
      .get()
  }

  /// The person's *Let the Orchestrator publish drafts*, *answer agents* or *merge runs* (`consent`) in the project.
  pub fn let_orchestrator(&self, consent: OrchestratorConsent, allowed: bool) {
    self
      .window
      .launch
      .workspace()
      .expect("the session started")
      .set_orchestrator_allows(&self.root, consent, allowed)
      .expect("the person's choice is kept");
  }

  /// The project's *Allow Autopilot for agents in this project*, as a person turns it on or off.
  pub fn allow_autopilot(&self, on: bool) {
    self
      .db()
      .save_allow_autopilot(if on { AllowAutopilot::ON } else { AllowAutopilot::OFF })
      .expect("Allow Autopilot saved");
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
    self.draft_with(pipeline, title, ("Read the readme.", &["The readme was read."]))
  }

  /// A draft task titled `title` on `pipeline` with `brief` and `criteria`; its key.
  pub fn draft_with(&self, pipeline: ResourceId, title: &str, (brief, criteria): (&str, &[&str])) -> String {
    let details = TaskDetails {
      title: named(title),
      brief: brief.into(),
      acceptance_criteria: criteria.iter().map(|criterion| (*criterion).into()).collect(),
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

/// An instruction for every agent of step *Implement* of `pipeline` when it starts, telling `text`.
fn instruction_body(pipeline: ResourceId, text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Step {
      pipeline,
      step: StepKey::new("implement").expect("key"),
    },
  )
  .expect("instruction")
}

/// A pipeline of one step by `builder`, reading, told `instructions`.
fn one_step(builder: ResourceId, instructions: &str) -> Pipeline {
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: named("Implement"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new(instructions).expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step]).expect("pipeline")
}

fn named(value: &str) -> Name {
  Name::new(value).expect("name")
}

/// The project's connections as `setup` has them, to the fake CLIs `fake` (Claude Code) and `fake_codex`, and what the
/// Orchestrator runs on: the Claude Code connection's id and the Codex one's.
fn connect(db: &mut ProjectDb, setup: Setup, (fake, fake_codex): (&Path, &Path)) -> (ConnectionId, ConnectionId) {
  let claude = connections::cli(AgentProvider::ClaudeCodeCli, "Claude · Kontur", fake);
  let codex = connections::cli(AgentProvider::CodexCli, "Codex · Kontur", fake_codex);
  match setup {
    Setup::NoConnection => {}
    Setup::NativeOnly => {
      let native = connections::native("Anthropic API · Kontur");
      db.create_connection(&native).expect("native connection");
    }
    Setup::Ready | Setup::Codex | Setup::NotChosen | Setup::NoRunner => {
      db.create_connection(&claude).expect("Claude Code connection");
      db.create_connection(&codex).expect("Codex connection");
    }
  }
  let published = match setup {
    Setup::Codex => Some((codex.id(), "gpt-5.5")),
    Setup::Ready | Setup::NoRunner => Some((claude.id(), "haiku")),
    Setup::NotChosen | Setup::NoConnection | Setup::NativeOnly => None,
  };
  if let Some((on, model)) = published {
    let settings = OrchestratorSettings::new(on, ModelId::new(model).expect("model"), None);
    db.publish_orchestrator(&settings, Timestamp::now()).expect("settings");
  }
  (claude.id(), codex.id())
}
