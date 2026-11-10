//! A window on a seeded Kontur (`examples/seed`) whose runner program does not exist, so a run never
//! starts: the form is driven by element id, as a person fills it in, and what it stored is read back from
//! the project's database.

use std::{fs, path::Path, time::Duration};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::TaskId,
  project::TaskKey,
  store::TaskStore,
  task::{Task, TaskDetails},
};
use orchester_store::ProjectDb;
use orchester_workspace::tasks::data::ProjectTasks;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

pub struct Form {
  pub window: Window,
  pub project: orchester_core::id::ProjectId,
  pub repo: std::path::PathBuf,
}

impl Form {
  /// The window on `scenario` (`live`, `tasks`), seeded in the window's own data folder.
  pub fn seeded(scenario: &str) -> Self {
    let data = Data::new();
    let fake_codex = data.dir.path().join("fake-codex");
    fs::write(&fake_codex, b"").expect("a stand-in for the fake Codex");
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: Some(fake_codex),
      now: orchester_core::time::Timestamp::now(),
    })
    .expect("the scenario seeds");
    let launch = data.launch();
    // A team scenario's resources are the team's configuration a stream would read into memory; these tests run none.
    if let Some(workspace) = launch.workspace() {
      crate::support::team_cache::keep_cached_team(workspace, &seeded.repo);
    }
    Self {
      window: Window::of(data, launch),
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  /// The window on Kontur without a pipeline (`support::Window::open`).
  pub fn bare() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self {
      window,
      project: crate::support::project_id(),
      repo,
    }
  }

  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.pass();
  }

  /// New task, once its fields are filled.
  pub fn new_task(&mut self) {
    self.go(ProjectPage::NewTask);
    self.window.until("New task's form", |window| {
      window.tree.get_element_by_id("task-form-title-input").is_some()
    });
  }

  /// Edit task on `key`, once its fields are filled, or the fields to choose between versions shown.
  pub fn edit(&mut self, key: &str) {
    self.go(ProjectPage::EditTask(key.parse().expect("a task key")));
    self.window.until("Edit task's form", |window| {
      window.tree.get_element_by_id("task-form-head").is_some()
    });
    self.until_readiness_read();
  }

  /// Waits until the bar no longer says the chosen pipeline's readiness is being read: Create & run is held back
  /// meanwhile, and the bar's line is not the one a test asks for.
  pub fn until_readiness_read(&mut self) {
    self
      .window
      .until_within(Duration::from_secs(10), "the pipelines' readiness", |window| {
        window.texts("task-form-reason-text").is_none_or(|reason| {
          let reason = reason.join("");
          !reason.starts_with("Reading whether") && !reason.contains("wait while the pipeline's readiness is read")
        })
      });
  }

  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(value);
    self.window.pass();
  }

  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// Opens the popover of the chip `id` (`task-form-limits`, `task-form-more`…), unless it is open.
  pub fn open(&mut self, id: &str) {
    let panel = match id {
      "task-form-pipeline"
      | "task-form-delivers"
      | "task-form-module"
      | "task-form-base-branch"
      | "task-form-points"
      | "task-form-priority" => {
        format!("{id}-menu")
      }
      _ => format!("{id}-popover"),
    };
    if !self.exists(&panel) {
      self.click(id);
    }
    self.window.until(&format!("{panel} opens"), |window| {
      window.tree.get_element_by_id(&panel).is_some()
    });
  }

  /// Chooses the pipeline whose title starts with `label` in the Pipeline chip's menu.
  pub fn choose_pipeline(&mut self, label: &str) {
    self.open("task-form-pipeline");
    for index in 0.. {
      let id = format!("task-form-pipeline-option-{index}");
      let Some(title) = self.window.texts(&format!("{id}-title")) else {
        break;
      };
      if title.join("").starts_with(label) {
        self.click(&id);
        self.until_readiness_read();
        return;
      }
    }
    panic!("the pipeline menu offers {label}");
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

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Where the element `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// The words of the element `id`, joined.
  pub fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  /// The one line at the action bar's start, if any.
  pub fn reasons(&mut self) -> Vec<String> {
    self
      .window
      .texts("task-form-reason-text")
      .map(|texts| vec![texts.join("")])
      .unwrap_or_default()
  }

  pub fn status(&mut self) -> String {
    self.text("task-form-status-label")
  }

  /// Sets the run budget and the time limit in the Limits chip's popover.
  pub fn limits(&mut self, budget: &str, time_limit: &str) {
    self.open("task-form-limits");
    self.set("task-form-budget-input", budget);
    self.set("task-form-time-limit-input", time_limit);
  }

  /// Fills a task that can run on `pipeline`.
  pub fn fill(&mut self, title: &str, pipeline: &str) {
    self.set("task-form-title-input", title);
    self.set("task-form-brief-input", "Record every delivery attempt.");
    self.click("task-form-add-criterion");
    self.set("task-form-criterion-1-input", "Every attempt is stored.");
    self.choose_pipeline(pipeline);
    self.limits("1,400,000", "30");
  }

  pub fn db(&self) -> ProjectDb {
    db(&self.repo)
  }

  pub fn task(&self, key: &str) -> Option<Task> {
    let key: TaskKey = key.parse().expect("a task key");
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| *task.key() == key)
  }
}

impl Form {
  /// Tells the window the project's tasks were saved and waits until the tasks it shows are ones read since, `read`
  /// holding of them. The feed keeps the tasks last read while it reads again, and a read already under way when the
  /// database changed ends with the earlier data, so a form opened at once would start from that.
  pub fn until_tasks_read(&mut self, read: impl Fn(&ProjectTasks) -> bool) {
    let feed = self.window.projects().feed(self.project);
    feed.saved();
    self.window.until("the project's tasks read again", |_| {
      feed.tasks.get().ready().is_some_and(&read)
    });
  }

  /// Changes the task `key` in the project's database, as automation or another app would, and has the
  /// window read the project's tasks again.
  pub fn change_elsewhere(&mut self, key: &str, change: impl FnOnce(&mut TaskDetails)) {
    let task = self.task(key).expect("the task is stored");
    let mut details = task.details().clone();
    change(&mut details);
    let saved = self
      .db()
      .update_task(task.id(), task.revision(), details)
      .expect("the change is stored")
      .saved()
      .expect("no conflict");
    assert!(saved.revision().get() > task.revision().get());
    let (id, revision) = (task.id(), saved.revision());
    self.until_tasks_read(|tasks| {
      tasks
        .tasks
        .iter()
        .any(|read| read.id() == id && read.revision() == revision)
    });
  }

  /// Changes the task `key` in the project's database without telling the window, as another app on
  /// the same project would: the form learns of it when it saves.
  pub fn change_unseen(&mut self, key: &str, change: impl FnOnce(&mut TaskDetails)) {
    let task = self.task(key).expect("the task is stored");
    let mut details = task.details().clone();
    change(&mut details);
    let _saved = self
      .db()
      .update_task(task.id(), task.revision(), details)
      .expect("the change is stored")
      .saved()
      .expect("no conflict");
  }

  /// Edits of the task `key` with the title `title`, kept by a form of an earlier visit, made on the
  /// task as it is stored now.
  pub fn keep_earlier(&self, key: &str, title: &str) {
    let task = self.task(key).expect("the task is stored");
    let details = task.details();
    let values = |title: &str| {
      serde_json::json!({
        "title": title,
        "brief": details.brief,
        "criteria": details.acceptance_criteria,
        "pipeline": details.pipeline.map(|pipeline| pipeline.to_string()),
        "budget": details.limits.budget().map(|budget| budget.get().to_string()).unwrap_or_default(),
        "time_limit": details
          .limits
          .max_duration()
          .map(|limit| (limit.as_secs() / 60).to_string())
          .unwrap_or_default(),
      })
    };
    let kept = serde_json::json!({
      "at": orchester_core::time::Timestamp::now().unix_millis(),
      "owner": "a form of an earlier visit",
      "values": values(title),
      "base_revision": task.revision().get(),
      "base": values(details.title.as_str()),
    });
    let key = format!("orchester-app.task_form.kept.{}.{}", self.project, task.id());
    let workspace = self.window.launch.workspace().expect("the session started");
    workspace
      .set_ui_state(&key, &kept.to_string())
      .expect("the edits are kept");
  }

  /// Waits until what is kept for `task` satisfies `done`.
  pub fn until_kept(&mut self, task: Option<TaskId>, what: &str, done: impl Fn(Option<&str>) -> bool) {
    let (project, key) = (
      self.project,
      task.map_or_else(|| "new".to_owned(), |task| task.to_string()),
    );
    let key = format!("orchester-app.task_form.kept.{project}.{key}");
    self.window.until(what, |window| {
      let workspace = window.launch.workspace().expect("the session started");
      done(workspace.ui_state(&key).expect("the window state reads").as_deref())
    });
  }
}

fn db(repo: &Path) -> ProjectDb {
  ProjectDb::open(repo).expect("the project database opens")
}
