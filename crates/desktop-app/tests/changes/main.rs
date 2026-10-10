//! The task page's Changes tab headless at 1440 × 900 on the seed's `changes` scenario (`examples/seed`): the
//! run's checkpointed files and what its worktree holds beyond them with their counts, the diff's line
//! numbers and painted token colors, choosing a file, the count on the tab's label, and every designed
//! state (`states`): no checkpoint yet, too large, worktree removed, a line shortened, a file in both
//! groups, a worktree read again, and changes that could not be read until Try again. The project's runner
//! program does not exist, so no agent is at work. Needs `git`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed is shared with its own tests, which read what this binary does not"
)]
mod kontur;
mod states;
#[path = "../support/mod.rs"]
mod support;

use std::path::PathBuf;

use kontur::{SeedRequest, scenarios};
use lurq::{
  app::{App, events::MouseButton},
  layout::{quad::QuadContent, text_style::FontWeight},
  node::color::Color,
};
use orchester_app::{
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskView},
  theme::{self, CodeText, Mode},
};
use orchester_core::{
  id::ProjectId,
  integration::WORKTREES_DIR,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use support::{Data, Window};

/// The window on the `changes` scenario, and where it was seeded.
struct Page {
  window: Window,
  project: ProjectId,
  repo: PathBuf,
}

impl Page {
  fn open() -> Self {
    let data = Data::new();
    let repo = data.dir.path().join("repo-changes");
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: repo.clone(),
      scenario: scenarios::named("changes").expect("the changes scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    })
    .expect("the scenario seeds");
    assert_eq!(
      seeded.app_database,
      data.database(),
      "the seed registers Kontur where the app reads"
    );
    let launch = data.launch();
    Self {
      window: Window::of(data, launch),
      project: seeded.project,
      repo,
    }
  }

  fn go(&mut self, path: &str) {
    let route = Route::parse(&format!("/project/{}/tasks/{path}", self.project)).expect("a task's route");
    self.window.navigation().go(route);
  }

  /// `key`'s Changes tab on `path`, its change from `source`.
  fn open_file(&mut self, key: &str, path: &str, source: ChangeSource) {
    let file = ChangedFile::new(path, source).expect("a path");
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::Changes(Some(file)));
    self.window.navigation().go(Route::Project(self.project, page));
  }

  /// Opens `key`'s Changes tab and waits until `id` shows.
  fn changes(&mut self, key: &str, id: &str) {
    self.go(&format!("{key}/changes"));
    self.wait_for(id);
  }

  fn wait_for(&mut self, id: &str) {
    self
      .window
      .until(id, |window| window.tree.get_element_by_id(id).is_some());
  }

  fn texts(&mut self, id: &str) -> Vec<String> {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} is shown"))
  }

  fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  fn tab_label(&mut self) -> String {
    self.texts("task-tab-changes-label").concat()
  }

  fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    let tree = &mut self.window.tree;
    tree.mouse_move(x, y);
    tree.mouse_down(x, y, MouseButton::Left);
    tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }

  /// Presses and releases `key` with no modifier.
  fn key(&mut self, key: &str) {
    let tree = &mut self.window.tree;
    tree.key_down_with_meta(key.to_owned(), key.to_owned(), false, false, false, false);
    tree.key_up_with_meta(key.to_owned(), key.to_owned(), false, false, false, false);
    self.window.pass();
  }

  /// The colors the text `text` is painted in, as the last layout paints it.
  fn painted_all(&self, text: &str) -> Vec<Color> {
    self
      .window
      .tree
      .painted_quads()
      .into_iter()
      .filter_map(|quad| match quad.content {
        QuadContent::Text {
          text: painted, style, ..
        } if painted == text => Some(style.color),
        _ => None,
      })
      .collect()
  }

  /// The color the text `text` is painted in, as the last layout paints it.
  fn painted(&self, text: &str) -> Option<Color> {
    self
      .window
      .tree
      .painted_quads()
      .into_iter()
      .find_map(|quad| match quad.content {
        QuadContent::Text {
          text: painted, style, ..
        } if painted == text => Some(style.color),
        _ => None,
      })
  }

  /// The texts of the tokens of class `class`.
  fn tokens(&mut self, class: &str) -> Vec<String> {
    self
      .window
      .tree
      .get_elements_by_class_name(class)
      .iter()
      .filter_map(|element| element.text_content().map(str::to_owned))
      .collect()
  }

  /// The worktree of `key`'s latest run.
  fn worktree(&self, key: &str) -> PathBuf {
    let db = ProjectDb::open(&self.repo).expect("the project database opens");
    let task = db
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task");
    let run = db
      .runs_of_task(task.id())
      .expect("runs")
      .into_iter()
      .last()
      .expect("a run")
      .expect("a readable run");
    self.repo.join(WORKTREES_DIR).join(run.id().to_string())
  }
}

#[test]
fn changes_lists_both_groups_with_counts_line_numbers_and_a_chosen_file() {
  let mut page = Page::open();
  page.go("KON-12/changes");
  page.wait_for("task-tab-changes");
  // While the changes are read, the label has no count.
  assert_eq!(page.tab_label(), "Changes");
  page.wait_for("changes-files");
  page.wait_for("changes-group-worktree");
  assert_eq!(page.texts("changes-group-checkpointed"), ["CHECKPOINTED", "4"]);
  let worktree = page.texts("changes-group-worktree");
  assert_eq!(worktree[..2], ["NOT CHECKPOINTED", "2"]);
  assert!(worktree[2].starts_with("In the worktree after "), "{worktree:?}");
  assert_eq!(page.tab_label(), "Changes 6");
  let toolbar = page.texts("changes-toolbar");
  assert!(toolbar.iter().any(|text| text == "Checkpointed"), "{toolbar:?}");
  assert!(
    toolbar
      .iter()
      .any(|text| text.starts_with("checkpoint after 03 Implement · ")),
    "{toolbar:?}"
  );
  assert!(
    toolbar.iter().any(|text| text.starts_with("4 files · +")),
    "{toolbar:?}"
  );
  let row = page.texts("changes-worktree-file:tests/webhooks/jitter.rs");
  assert_eq!(row[0], "A");
  assert!(row.iter().any(|text| text.starts_with('+')), "{row:?}");
  assert!(
    !row.iter().any(|text| text.starts_with('−')),
    "an added file has no removed count: {row:?}"
  );
  // The first file of the checkpointed group, in git's order, shows: an added migration, its lines
  // numbered on the new side only, its long path cut near its middle as the design cuts it.
  let line = page.texts("changes-diff-line");
  assert_eq!(line[..2], ["1", "+"], "no old number, new number 1: {line:?}");
  let migration = page
    .texts("changes-file:migrations/0042_delivery_attempts.sql-path")
    .concat();
  assert_eq!(migration, "migrations/0042_de…_attempts.sql");
  // Choosing a worktree file puts it in the path, in place of the current entry.
  page.click("changes-worktree-file:src/webhooks/log.rs");
  page.window.until("the worktree file", |window| {
    window
      .texts("changes-diff-source")
      .is_some_and(|texts| texts.concat().starts_with("not checkpointed"))
  });
  let Route::Project(_, ProjectPage::Task(_, TaskView::Changes(Some(file)))) = page.window.navigation().route_now()
  else {
    panic!("the Changes tab names its file");
  };
  assert_eq!(
    (file.path(), file.source()),
    ("src/webhooks/log.rs", ChangeSource::Worktree)
  );
  assert_eq!(page.texts("changes-diff-path"), ["src/webhooks/log.rs"]);
  // Leaving the tab and opening it again opens the file it showed.
  page.click("task-tab-summary");
  page.wait_for("task-summary");
  page.click("task-tab-changes");
  page.wait_for("changes-diff-path");
  assert_eq!(page.texts("changes-diff-path"), ["src/webhooks/log.rs"]);
  // ArrowDown and ArrowUp choose the next and the previous file of the list.
  page.key("ArrowDown");
  page.window.until("the next file", |window| {
    window
      .texts("changes-diff-path")
      .is_some_and(|texts| texts == ["tests/webhooks/jitter.rs"])
  });
  page.key("ArrowUp");
  page.key("ArrowUp");
  page.window.until("two files up", |window| {
    window
      .texts("changes-diff-path")
      .is_some_and(|texts| texts == ["tests/webhooks/retry.rs"])
  });
}

#[test]
fn changes_paints_keywords_and_numbers_and_leaves_operators_plain() {
  let mut page = Page::open();
  page.open_file("KON-12", "src/webhooks/sender.rs", ChangeSource::Checkpoint);
  page.wait_for("changes-diff-line");
  page.window.until("highlighted lines", |window| {
    !window.tree.get_elements_by_class_name("code-keyword").is_empty()
  });
  let keywords = page.tokens("code-keyword");
  for keyword in ["let", "self", "await", "loop"] {
    assert!(
      keywords.iter().any(|token| token == keyword),
      "{keyword} in {keywords:?}"
    );
    assert_eq!(
      page.painted(keyword),
      Some(Color::from_hex("#8AB4F8")),
      "{keyword} in code_keyword"
    );
  }
  assert!(page.tokens("code-constant").iter().any(|token| token == "0"));
  assert_eq!(
    page.painted("0"),
    Some(Color::from_hex("#C3A6F7")),
    "a number in code_number"
  );
  let plain = page.tokens("code-plain");
  for operator in [" = ", "+=", "&&", "=>", "Ok("] {
    let token = plain
      .iter()
      .find(|text| text.contains(operator))
      .unwrap_or_else(|| panic!("{operator} is in a plain token: {plain:?}"));
    assert_eq!(
      page.painted(token),
      Some(Color::from_hex("#EDEDF0")),
      "{token:?} in text_primary"
    );
  }
}

#[test]
fn changes_text_roles_follow_the_design() {
  let mut app = App::new();
  theme::install(&mut app, Mode::Dark);
  let theme = app.theme();
  for (role, (size, weight, family, color, tracking)) in [
    (
      CodeText::ChangeType,
      (11.0, FontWeight::SemiBold, "Geist Mono", "#B3B3BA", 0.0),
    ),
    (
      CodeText::DiffPath,
      (12.5, FontWeight::Medium, "Geist Mono", "#EDEDF0", 0.0),
    ),
    (
      CodeText::GroupHeading,
      (10.5, FontWeight::Medium, "Geist", "#8A8A92", 0.5),
    ),
    (
      CodeText::GroupCount,
      (10.5, FontWeight::Normal, "Geist Mono", "#8A8A92", 0.0),
    ),
  ] {
    let style = theme.typography_style(role.role());
    assert_eq!(
      (
        style.font_size,
        style.weight,
        &*style.font_family,
        style.color,
        style.letter_spacing
      ),
      (size, weight, family, Color::from_hex(color), tracking),
      "{role:?}"
    );
  }
}
