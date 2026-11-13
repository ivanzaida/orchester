//! A private project and its pipeline editor; no runners or installed data.
use std::{path::PathBuf, time::Duration};

use orchester_api::test_support::team_server::TeamServer;
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, ResourceKind, Slug, StepKind, Tool, ToolAction},
  sharing::TaskRevision,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  sharing::Servers,
  team_config::{Changed, TeamResource, TeamVersion},
};

use crate::support::{
  Data, Window, project_id,
  stand_ins::StandIns,
  team_link::{self, STAND_IN_SLUG},
};

pub struct Editor {
  pub window: Window,
  repo: PathBuf,
  /// The stand-in team server a team project is shared on, held while the window is open; `None` for a local project.
  pub team: Option<TeamServer>,
}
impl Editor {
  pub fn new() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self {
      window,
      repo,
      team: None,
    }
  }
  /// The editor of a project shared with the stand-in team server, which keeps the step's own page: the Pipelines hub's
  /// panel shows a step of a local project, and a team project's pipelines are still edited here. Its pipelines are the
  /// team server's, and a save goes there (D-043).
  pub fn team() -> Self {
    let mut editor = Self::new();
    editor.team = Some(team_link::share_on_stand_in(&editor.window, &editor.repo));
    editor
  }
  /// The editor of a team project ([`Self::team`]) with the stand-in pickers, which hand over the folder a test chose.
  pub fn with_stand_ins() -> (Self, StandIns) {
    let data = Data::new();
    let launch = data.launch();
    data.add_kontur(launch.workspace().expect("the session started"));
    let (window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    let repo = window.data.dir.path().join("repo-kontur");
    let team = Some(team_link::share_on_stand_in(&window, &repo));
    (Self { window, repo, team }, stand_ins)
  }
  pub fn repo(&self) -> &std::path::Path {
    &self.repo
  }
  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(project_id(), page));
    self.window.until("project page", |w| {
      w.tree.get_element_by_id("screen-pipelines").is_some()
        || w.tree.get_element_by_id("screen-pipeline-hub").is_some()
    });
  }
  pub fn open(&mut self, id: ResourceId) {
    self.go(ProjectPage::Pipeline(id, Default::default()));
    self.until("editor", |e| e.exists("pipeline-add-step"));
  }
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        self.arm();
        return;
      }
      assert!(
        std::time::Instant::now() < deadline,
        "timed out: {what}; {:?}",
        self.window.texts("screen-pipelines")
      );
      std::thread::sleep(Duration::from_millis(5));
    }
  }
  pub fn arm(&mut self) {
    for _ in 0..3 {
      std::thread::sleep(Duration::from_millis(5));
      self.window.tree.tick_futures();
      self.window.pass();
    }
  }
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("input {id}"))
      .set_value(value);
    self.window.pass();
  }
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("element {id}"))
      .click();
    self.window.pass();
    self.window.pass();
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).expect("element").join(" ")
  }
  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("database")
  }
  /// What the team's configuration in memory holds of `id`: a team project keeps no copy of it in its database.
  pub fn held(&self, id: ResourceId) -> Option<TeamResource> {
    let workspace = self.window.launch.workspace().expect("the session");
    let read = workspace.team_read(&self.repo);
    read.config().and_then(|config| config.resource(id).cloned())
  }
  /// What `id` stores: its latest version's body (a pipeline holds no draft, D-032), in the project's database for a
  /// local project and in memory for a team one.
  pub fn stored(&self, id: ResourceId) -> Pipeline {
    if self.team.is_some() {
      let held = self.held(id).expect("memory holds the pipeline");
      return held.body::<Pipeline>().expect("a version").expect("a pipeline");
    }
    self
      .db()
      .latest_version::<Pipeline>(id)
      .expect("read")
      .expect("a version")
      .body()
      .clone()
  }
  /// The number of `id`'s latest version.
  pub fn version(&self, id: ResourceId) -> u32 {
    if self.team.is_some() {
      let held = self.held(id).expect("memory holds the pipeline");
      return held.number().expect("a version").get();
    }
    self
      .db()
      .latest_version::<Pipeline>(id)
      .expect("read")
      .expect("a version")
      .number()
      .get()
  }
  /// The name `id` has now, and the slug it keeps.
  pub fn named(&self, id: ResourceId) -> (String, String) {
    if self.team.is_some() {
      let held = self.held(id).expect("memory holds the pipeline");
      return (held.name.as_str().to_owned(), held.slug);
    }
    let resource = self.db().resource(id).expect("read").expect("the pipeline");
    (resource.name().as_str().to_owned(), resource.slug().as_str().to_owned())
  }
  /// Every version of `id`, oldest first: the project's database's in a local project, the team server's in a team one.
  pub fn versions(&self, id: ResourceId) -> Vec<Pipeline> {
    let Some(team) = &self.team else {
      let versions = self.db().versions::<Pipeline>(id).expect("read");
      return versions.iter().map(|version| version.body().clone()).collect();
    };
    let held = team.resources(STAND_IN_SLUG);
    let found = held.into_iter().find(|(resource, _)| resource["id"] == id.to_string());
    let (_, versions) = found.expect("the team server holds the pipeline");
    let body = |version: serde_json::Value| serde_json::from_value(version["body"].clone()).expect("a pipeline");
    versions.into_iter().map(body).collect()
  }
  /// The versions of team pipeline `id` as the page's history reads them from the team server.
  pub fn team_history(&self, id: ResourceId) -> orchester_workspace::pipelines::history::VersionHistory {
    let servers = self.window.launch.services().expect("the session").servers.clone();
    let read = servers.team_pipeline_history(&self.repo, id);
    self
      .window
      .data
      .runtime
      .block_on(read)
      .expect("the team server's versions")
  }
  /// Another editor publishes `id`'s next version, described as `description`, behind the page.
  pub fn publish_behind(&self, id: ResourceId, description: &str) {
    let body = self.stored(id).with_description(description.into());
    self.publish_next(id, body);
  }
  /// Publishes `body` as `id`'s next version behind the page: here in a local project, at the team server in a team
  /// project, which numbers it and keeps it here.
  pub fn publish_next(&self, id: ResourceId, body: Pipeline) {
    let latest = self.version(id);
    let based_on =
      orchester_core::resource::VersionNumber::new(std::num::NonZeroU32::new(latest).expect("a stored version number"));
    if self.team.is_none() {
      self
        .db()
        .publish_pipeline_edit(id, &body, based_on, None, Timestamp::now())
        .expect("their version");
      return;
    }
    self
      .on_worker(move |servers, repo| servers.publish_team_pipeline(&repo, id, based_on, &body))
      .expect("the team server numbers their version");
  }
  /// Runs `work` with the session's servers on a worker of the window's runtime, as the app does.
  fn on_worker<T: Send + 'static>(&self, work: impl FnOnce(Servers, PathBuf) -> T + Send + 'static) -> T {
    let servers = self.window.launch.services().expect("the session").servers.clone();
    let repo = self.repo.clone();
    self
      .window
      .data
      .runtime
      .block_on(async move { tokio::task::spawn_blocking(move || work(servers, repo)).await })
      .expect("the work finished")
  }
  /// Keeps pipeline `name` under `slug` with `body` as its first version: here in a local project, made by the team
  /// server in a team project.
  fn first_version(&self, (slug, name): (&str, &str), body: &Pipeline) -> ResourceId {
    if self.team.is_none() {
      let (id, _) = self
        .db()
        .published_pipeline_for_test(slug, name, body, Timestamp::now())
        .expect("valid pipeline fixture");
      return id;
    }
    let (slug, name) = (
      Slug::new(slug).expect("valid pipeline fixture"),
      Name::new(name).expect("valid pipeline fixture"),
    );
    let body = body.clone();
    self
      .on_worker(move |servers, repo| servers.create_team_pipeline(&repo, (slug, name), &body))
      .expect("the team server makes it")
  }
  pub fn selected(&self) -> ResourceId {
    match self.window.navigation().route_now() {
      Route::Project(_, ProjectPage::Pipeline(id, _)) => id,
      other => panic!("pipeline route: {other:?}"),
    }
  }
  /// Chooses `label` in the select `id`, or in the menu the field `id` opens (the step's Executor).
  pub fn choose(&mut self, id: &str, label: &str) {
    if let Some(mut select) = self.window.tree.get_element_by_id_mut(id).and_then(|e| e.as_select()) {
      let labels = select.labels();
      let index = labels
        .iter()
        .position(|option| &**option == label)
        .unwrap_or_else(|| panic!("option {label} in {id}: {labels:?}"));
      select.commit(index);
      self.window.pass();
      return;
    }
    self.click(id);
    for index in 0.. {
      let option = format!("{id}-option-{index}");
      assert!(self.exists(&option), "option {label} in {id}");
      if self.text(&format!("{option}-label")) == label {
        self.click(&option);
        return;
      }
    }
  }
  /// Removes the one step of the open pipeline's edit ([`Self::empty`]'s), as a person does: with no steps the page
  /// offers the starter's (*Use starter*). A stored pipeline always has a step (D-032).
  pub fn remove_only_step(&mut self) {
    self.click("pipeline-step-step");
    self.click("step-remove");
    self.until("remove question", |e| e.exists("step-remove-confirm"));
    self.click("step-remove-confirm");
    self.until("no steps", |e| e.exists("pipeline-use-starter"));
  }
  /// Presses *Save* once no save is under way (a team pipeline's waits for the server, and Save is off meanwhile), and
  /// allows what it asks a person to allow, if it asks (*Saving needs your approval*).
  pub fn save(&mut self) {
    self.until("no save under way", |e| {
      !e.text("pipeline-save-status").starts_with("Saving")
    });
    self.click("pipeline-save");
    self.arm();
    if self.exists("pipeline-save-question-confirm") {
      self.arm();
      self.click("pipeline-save-question-confirm");
    }
  }
  pub fn profile(&self) {
    use orchester_core::{
      id::ConnectionId,
      resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
    };
    if self.team.is_some() {
      self.team_profile();
      return;
    }
    let profile = AgentProfile::new(
      Name::new("Reviewer").expect("valid pipeline fixture"),
      "".into(),
      PromptText::new("").expect("valid pipeline fixture"),
      ConnectionId::generate(),
      ModelId::new("test-model").expect("valid pipeline fixture"),
      ProviderSettings::default(),
    );
    let mut db = self.db();
    let id = db
      .create_resource(
        Slug::new("reviewer").expect("valid pipeline fixture"),
        Name::new("Reviewer").expect("valid pipeline fixture"),
        &profile,
        Timestamp::now(),
      )
      .expect("valid pipeline fixture")
      .id();
    db.publish_draft::<AgentProfile>(id, Timestamp::now())
      .expect("valid pipeline fixture");
  }
  pub fn gated(&self) -> bool {
    self.window.link.input_gate().protecting()
  }
  /// Pipeline `name` with the fewest steps a stored pipeline holds: one, *Step*, that runs a published tool. A
  /// pipeline is stored only as published versions, whose steps all run something (D-032), so none is empty.
  pub fn empty(&self, name: &str) -> ResourceId {
    let tool = self.tool("fixture-step", "Fixture step");
    let mut body = PipelineEdit::new(Pipeline::new(vec![]).expect("valid pipeline fixture"));
    let key = body.add_step(Name::new("Step").expect("valid pipeline fixture"), None);
    body.update(&key, |s| s.kind = StepKind::Tool { tool: Some(tool) });
    let slug = Slug::from_name(name).expect("valid pipeline fixture");
    self.first_version((slug.as_str(), name), &body.validate().expect("valid pipeline fixture"))
  }
  pub fn published(&self) -> ResourceId {
    let tool_id = self.tool("checks", "Checks");
    let mut body = PipelineEdit::new(Pipeline::new(vec![]).expect("valid pipeline fixture"));
    let key = body.add_step(Name::new("Check").expect("valid pipeline fixture"), None);
    body.update(&key, |s| {
      s.kind = StepKind::Tool { tool: Some(tool_id) };
      s.approval_gate = true;
    });
    self.first_version(
      ("delivery", "Delivery"),
      &body.validate().expect("valid pipeline fixture"),
    )
  }
}

/// What a team project's pipelines run: the team's, kept in memory as a read of the team server holds them. A team
/// pipeline's steps may run only the team's profiles and tools (D-043), and the project's database keeps no copy.
impl Editor {
  /// Keeps a `kind` named `name` under `slug` in the team's configuration, with `body` as its first version, or with no
  /// version yet without one; its id.
  pub fn the_teams(
    &self,
    (kind, slug, name): (ResourceKind, &str, &str),
    body: Option<serde_json::Value>,
  ) -> ResourceId {
    let id = ResourceId::generate();
    let resource = TeamResource {
      id,
      kind,
      name: Name::new(name).expect("valid pipeline fixture"),
      slug: slug.to_owned(),
      archived: false,
      revision: TaskRevision::FIRST,
      latest: body.map(|body| TeamVersion {
        number: orchester_core::resource::VersionNumber::FIRST,
        body,
        published_by: "Alice Moreau".to_owned(),
        published_at: Timestamp::now(),
      }),
      updated: Changed {
        by: "Alice Moreau".to_owned(),
        at: Timestamp::now(),
      },
    };
    let workspace = self.window.launch.workspace().expect("the session");
    assert!(
      workspace.keep_team_resource_for_test(&self.repo, resource),
      "the team's configuration is read"
    );
    id
  }

  /// The team profile *Reviewer*, which runs Codex's `test-model`.
  fn team_profile(&self) {
    use orchester_core::{
      connection::AgentProvider,
      resource::{ModelId, PromptText, ProviderSettings, TeamAgentProfile},
    };
    let team = TeamAgentProfile::new(
      Name::new("Reviewer").expect("valid pipeline fixture"),
      String::new(),
      PromptText::new("").expect("valid pipeline fixture"),
      (
        AgentProvider::CodexCli,
        ModelId::new("test-model").expect("valid pipeline fixture"),
        ProviderSettings::default(),
      ),
    );
    let body = serde_json::to_value(&team).expect("valid pipeline fixture");
    self.the_teams((ResourceKind::AgentProfile, "reviewer", "Reviewer"), Some(body));
  }

  /// The tool `name` under `slug`, which runs `cargo`, published: the project's own in a local project, the team's in
  /// a team one. The one already there under `slug` is used again.
  fn tool(&self, slug: &str, name: &str) -> ResourceId {
    let body = Tool::new(
      "",
      ToolAction::Command(CommandAction::new(
        CommandProgram::new("cargo").expect("valid pipeline fixture"),
        vec![],
      )),
      None,
    )
    .expect("valid pipeline fixture");
    if self.team.is_some() {
      let workspace = self.window.launch.workspace().expect("the session");
      let read = workspace.team_read(&self.repo);
      let held = read.config().and_then(|config| {
        let tools = config.resources(ResourceKind::Tool);
        tools.into_iter().find(|tool| tool.slug == slug).map(|tool| tool.id)
      });
      let body = serde_json::to_value(&body).expect("valid pipeline fixture");
      return held.unwrap_or_else(|| self.the_teams((ResourceKind::Tool, slug, name), Some(body)));
    }
    let mut db = self.db();
    let kept = db.resources(ResourceKind::Tool).expect("valid pipeline fixture");
    if let Some(tool) = kept.into_iter().find(|tool| tool.slug().as_str() == slug) {
      return tool.id();
    }
    let made = db.create_published(
      Slug::new(slug).expect("valid pipeline fixture"),
      Name::new(name).expect("valid pipeline fixture"),
      &body,
      Timestamp::now(),
    );
    made.expect("valid pipeline fixture").0.id()
  }
}

impl Editor {
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id(id)
      .expect("text input")
      .text_content()
      .unwrap_or_default()
      .to_owned()
  }

  pub fn focus(&mut self, id: &str) {
    self.window.tree.get_element_by_id_mut(id).expect("input").focus();
    self.window.pass();
  }

  pub fn key(&mut self, key: &str, control: bool) {
    let ctrl = control && !cfg!(target_os = "macos");
    let meta = control && cfg!(target_os = "macos");
    let code = if key.len() == 1 {
      format!("Key{}", key.to_uppercase())
    } else {
      key.to_owned()
    };
    self
      .window
      .tree
      .key_down_with_meta(key.into(), code.clone(), false, ctrl, false, meta);
    self
      .window
      .tree
      .key_up_with_meta(key.into(), code, false, ctrl, false, meta);
    self.window.pass();
  }
}
