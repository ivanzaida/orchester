use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{InstructionsTab, InstructionsView, ProjectPage, Route};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText},
};
use orchester_workspace::instructions::{
  data::{InstructionEntry, ProjectInstructions},
  editor,
};

use super::support::{Data, Window, project_id};

/// The scope tab a rule was in before the list had a group per scope.
fn scope_tab(scope: &InstructionScope) -> InstructionsTab {
  match scope {
    InstructionScope::Project => InstructionsTab::Project,
    InstructionScope::AgentRole { .. } => InstructionsTab::Roles,
    InstructionScope::Pipeline { .. } => InstructionsTab::Pipelines,
    InstructionScope::Step { .. } => InstructionsTab::Steps,
  }
}

pub struct Instructions {
  pub window: Window,
  pub root: PathBuf,
}
impl Instructions {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/instructions");
    std::fs::create_dir_all(&scratch).expect("valid instruction fixture");
    let data = Data::at(
      tempfile::tempdir_in(scratch).expect("valid instruction fixture"),
      tokio::runtime::Runtime::new().expect("valid instruction fixture"),
    );
    let launch = data.launch();
    let project = data.add_kontur(launch.workspace().expect("valid instruction fixture"));
    Self {
      root: project.repo_root,
      window: Window::of(data, launch),
    }
  }
  pub fn rule(&self, name: &str, text: &str, scope: InstructionScope, published: bool) -> ResourceId {
    let entry = editor::create_in(
      &self.root,
      Name::new(name).expect("valid instruction fixture"),
      &body(text, scope),
    )
    .expect("valid instruction fixture");
    if published {
      editor::publish_in(&self.root, &entry).expect("valid instruction fixture");
    }
    entry.id
  }
  /// Opens Instructions as it opened before the Agents hub: on the first project rule, or the list without one.
  pub fn go(&mut self) {
    self.list();
    self.open_tab(InstructionsTab::Project);
  }
  /// The list of every rule (`02`).
  pub fn list(&mut self) {
    self.window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::Instructions(Default::default()),
    ));
    self.until("instructions read", |s| {
      s.exists("instructions-new") && (s.exists("instruction-library") || s.exists("instructions-empty"))
    });
  }
  /// The first rule of `tab`'s scope, as its tab opened it before the list had a group per scope; the list when
  /// the scope has none.
  pub fn open_tab(&mut self, tab: InstructionsTab) {
    let mut rules: Vec<InstructionEntry> = self
      .listed()
      .entries
      .iter()
      .filter(|e| scope_tab(e.body.scope()) == tab)
      .cloned()
      .collect();
    rules.sort_by(|a, b| a.name.cmp(&b.name));
    let first = rules.first().map(|e| e.id);
    self.window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::Instructions(InstructionsView::rule(tab, first)),
    ));
    let name = rules.first().map(|e| e.name.to_string());
    self.until("the rule or the list", |s| {
      // Unsaved edits ask before the page leaves them.
      s.exists("instruction-leave-question")
        || match &name {
          Some(name) => s.exists("instruction-prompt") && s.exists("instruction-title") && s.text("instruction-title") == *name,
          None => s.exists("instruction-library") || s.exists("instructions-empty"),
        }
    });
  }
  pub fn until(&mut self, what: &str, mut predicate: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if predicate(self) {
        return;
      }
      assert!(
        std::time::Instant::now() < deadline,
        "{what}: {:?}",
        self.window.texts("screen-instructions")
      );
      std::thread::sleep(Duration::from_millis(5));
    }
  }
  /// Presses `id`; `instruction-tab-N`, a scope tab before the hub, opens that scope's first rule.
  pub fn click(&mut self, id: &str) {
    if let Some(index) = id.strip_prefix("instruction-tab-").and_then(|n| n.parse::<usize>().ok()) {
      return self.open_tab(InstructionsTab::ALL[index]);
    }
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id}"))
      .click();
    self.window.pass();
    self.window.pass();
  }
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id}"))
      .set_value(value);
    self.window.pass();
  }
  pub fn focus_prompt(&mut self) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut("instruction-prompt")
      .and_then(|element| element.bounds())
      .expect("prompt laid out");
    let (x, y) = (bounds.x + 5.0, bounds.y + 5.0);
    self.window.tree.mouse_move(x, y);
    self.window.tree.mouse_down(x, y, lurq::app::events::MouseButton::Left);
    self.window.tree.mouse_up(x, y, lurq::app::events::MouseButton::Left);
    self.window.pass();
  }
  pub fn key(&mut self, key: &str, code: &str, shift: bool, control: bool) {
    self
      .window
      .tree
      .key_down(key.into(), code.into(), shift, control, false);
    self.window.pass();
  }
  /// What the person typed into the input `id`.
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id}"))
      .value()
  }
  pub fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_select())
      .expect("valid instruction fixture");
    let index = select
      .labels()
      .iter()
      .position(|value| &**value == label)
      .expect("valid instruction fixture");
    select.commit(index);
    self.window.pass();
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_else(|| panic!("{id}")).join(" ")
  }
  pub fn listed(&self) -> ProjectInstructions {
    ProjectInstructions::load_in(&self.root).expect("valid instruction fixture")
  }
  pub fn entry(&self, id: ResourceId) -> InstructionEntry {
    self.listed().entry(id).expect("valid instruction fixture").clone()
  }
  pub fn pump(&mut self) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_millis(1000) {
      self.window.tree.tick_futures();
      self.window.pass();
      std::thread::sleep(Duration::from_millis(5));
    }
  }
}
pub fn body(text: &str, scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid instruction fixture"),
    AgentTargets::All,
    std::collections::BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("valid instruction fixture")
}
