//! A private project and its pipeline editor; no runners or installed data.
use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{CommandAction, CommandProgram, Pipeline, PipelineEdit, Slug, StepKind, Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::support::{Data, Window, project_id, stand_ins::StandIns};

pub struct Editor {
  pub window: Window,
  repo: PathBuf,
}
impl Editor {
  pub fn new() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self { window, repo }
  }
  /// The editor with the stand-in pickers, which hand over the folder a test chose.
  pub fn with_stand_ins() -> (Self, StandIns) {
    let data = Data::new();
    let launch = data.launch();
    data.add_kontur(launch.workspace().expect("the session started"));
    let (window, stand_ins) = Window::launched_with_stand_ins(data, launch);
    let repo = window.data.dir.path().join("repo-kontur");
    (Self { window, repo }, stand_ins)
  }
  pub fn repo(&self) -> &std::path::Path {
    &self.repo
  }
  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(project_id(), page));
    self.window.until("project page", |w| {
      w.tree.get_element_by_id("screen-pipelines").is_some()
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
  pub fn selected(&self) -> ResourceId {
    match self.window.navigation().route_now() {
      Route::Project(_, ProjectPage::Pipeline(id, _)) => id,
      other => panic!("pipeline route: {other:?}"),
    }
  }
  /// Chooses `label` in the select `id`, or in the menu the field `id` opens (the step's Executor).
  pub fn choose(&mut self, id: &str, label: &str) {
    if let Some(mut select) = self.window.tree.get_element_by_id_mut(id).and_then(|e| e.as_select()) {
      let index = select
        .labels()
        .iter()
        .position(|option| &**option == label)
        .expect("option");
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
  pub fn profile(&self) {
    use orchester_core::{
      id::ConnectionId,
      resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
    };
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
  pub fn empty(&self, name: &str) -> ResourceId {
    self
      .db()
      .create_resource(
        Slug::from_name(name).expect("valid pipeline fixture"),
        Name::new(name).expect("valid pipeline fixture"),
        &Pipeline::new(Vec::new()).expect("valid pipeline fixture"),
        Timestamp::now(),
      )
      .expect("valid pipeline fixture")
      .id()
  }
  pub fn published(&self) -> ResourceId {
    let mut db = self.db();
    let tool = Tool::new(
      "",
      ToolAction::Command(CommandAction::new(
        CommandProgram::new("cargo").expect("valid pipeline fixture"),
        vec![],
      )),
      None,
    )
    .expect("valid pipeline fixture");
    let tool_id = db
      .create_resource(
        Slug::new("checks").expect("valid pipeline fixture"),
        Name::new("Checks").expect("valid pipeline fixture"),
        &tool,
        Timestamp::now(),
      )
      .expect("valid pipeline fixture")
      .id();
    db.publish_draft::<Tool>(tool_id, Timestamp::now())
      .expect("valid pipeline fixture");
    let mut body = PipelineEdit::new(Pipeline::new(vec![]).expect("valid pipeline fixture"));
    let key = body.add_step(Name::new("Check").expect("valid pipeline fixture"), None);
    body.update(&key, |s| {
      s.kind = StepKind::Tool { tool: Some(tool_id) };
      s.approval_gate = true;
    });
    let id = db
      .create_resource(
        Slug::new("delivery").expect("valid pipeline fixture"),
        Name::new("Delivery").expect("valid pipeline fixture"),
        &body.validate().expect("valid pipeline fixture"),
        Timestamp::now(),
      )
      .expect("valid pipeline fixture")
      .id();
    db.publish_draft::<Pipeline>(id, Timestamp::now())
      .expect("valid pipeline fixture");
    id
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
