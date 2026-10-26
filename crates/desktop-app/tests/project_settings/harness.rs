//! A project and window of the test's own, with settings driven by element id.
use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};
use orchester_core::{
  connection::{AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::ConnectionId,
  name::Name,
  store::{ConnectionStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::support::{Window, project_id};

pub struct Settings {
  pub window: Window,
  pub repo: PathBuf,
  editing: String,
}
impl Settings {
  /// Project settings on Runs, where Auto mode, the limits and the environment are.
  pub fn new(reviewer: bool) -> Self {
    let mut window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    if reviewer {
      add_reviewer(&repo);
    }
    window.tree.resize(1440, 1375);
    window.navigation().go(Route::Project(
      project_id(),
      ProjectPage::Settings(ProjectSettingsTab::Runs),
    ));
    window.until("settings Runs", |window| window.texts("settings-auto").is_some());
    Self {
      window,
      repo,
      editing: String::new(),
    }
  }
  /// The tab that holds what `name` names, once `ready` shows: Limits and Environment are on Runs, Sources and the code
  /// index on Context. The environment's variables show once its row's Edit opens them.
  pub fn tab(&mut self, name: &str, ready: &str) {
    let tab = match name {
      "limits" | "environment" => "runs",
      "sources" | "codeIndex" | "code_index" => "context",
      tab => tab,
    };
    self.click(&format!("settings-tab-{tab}"));
    if ready == "settings-environment-variables" {
      self.until("the environment's row", |s| {
        s.exists(ready) || s.exists("settings-environment-edit")
      });
      if !self.exists(ready) {
        self.click("settings-environment-edit");
      }
    }
    self.until("the tab", |s| s.exists(ready));
  }
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(
        std::time::Instant::now() < deadline,
        "timed out waiting for {what}; status={:?}; limits={:?}",
        self.window.texts("project-settings-save-status"),
        self.db().project_limits().expect("limits")
      );
      std::thread::sleep(Duration::from_millis(5));
    }
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }
  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("project database")
  }
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.window.pass();
  }
  pub fn set(&mut self, id: &str, text: &str) {
    self.editing = id.to_owned();
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .focus();
    self.window.pass();
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .expect("text input")
      .set_value(text);
    self.window.pass();
  }
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .expect("text input")
      .value()
  }
  pub fn leave(&mut self) {
    // Move to another focusable control, as clicking out of the edited field does.
    let target = if self.editing == "settings-environment-variables" {
      "settings-open-secrets"
    } else if self.editing == "settings-agent-slots" {
      "settings-time-limit"
    } else {
      "settings-agent-slots"
    };
    self
      .window
      .tree
      .get_element_by_id_mut(target)
      .expect("next control")
      .focus();
    self.window.pass();
  }
  /// Chooses `label` in the select `id`, or in the choice field `id` (`ui::choice_field`) by opening its menu and
  /// clicking the row whose first words are `label`.
  pub fn choose(&mut self, id: &str, label: &str) {
    if let Some(mut select) = self.window.tree.get_element_by_id_mut(id).and_then(|e| e.as_select()) {
      let i = select.labels().iter().position(|v| &**v == label).expect("option");
      select.commit(i);
      self.window.pass();
      return;
    }
    self.click(id);
    let option = (0..)
      .map(|index| format!("{id}-option-{index}"))
      .map_while(|option| {
        let first = self.window.texts(&option)?.first().cloned()?;
        Some((option, first))
      })
      .find(|(_, first)| first == label)
      .map(|(option, _)| option)
      .unwrap_or_else(|| panic!("{id} offers {label}"));
    self.click(&option);
  }
  pub fn saved(&mut self) {
    self.until("saved readback", |s| {
      s.text("project-settings-save-status") == "All changes saved"
    });
  }
  /// The reviewer's model as auto mode stores it; empty while none is chosen. The page says it in its (i).
  pub fn reviewer_model(&self) -> String {
    self
      .db()
      .auto_mode_settings()
      .expect("auto mode")
      .reviewer
      .map(|choice| choice.model.as_str().to_owned())
      .unwrap_or_default()
  }
  pub fn outside(&self, change: orchester_workspace::tasks::outside::Outside) {
    self
      .window
      .launch
      .services()
      .expect("services")
      .changes
      .record(project_id(), change);
  }
}
fn add_reviewer(root: &std::path::Path) {
  let mut db = ProjectDb::open(root).expect("database");
  let mut agent = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude Code").expect("name"),
    AgentProvider::ClaudeCodeCli,
    std::env::current_exe().expect("test program"),
    Timestamp::now(),
  )
  .expect("connection");
  agent
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(
          CatalogSource::DocumentedAliases,
          vec![
            orchester_core::connection::CatalogModel::new(
              orchester_core::resource::ModelId::new("sonnet").expect("model"),
              vec!["low".into(), "high".into()],
            ),
            orchester_core::connection::CatalogModel::new(
              orchester_core::resource::ModelId::new("haiku").expect("model"),
              Vec::new(),
            ),
          ],
        ),
      },
    })
    .expect("valid check");
  db.create_connection(&agent.into_connection()).expect("save reviewer");
}
