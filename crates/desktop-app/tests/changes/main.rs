//! The task page's Changes tab headless at 1440 × 900 on the seed's `changes` scenario (`examples/seed`): the
//! run's checkpointed files and what its worktree holds beyond them with their counts, the diff's line
//! numbers and token colors, choosing a file, the count on the tab's label, and every designed state: no
//! checkpoint yet, too large, worktree removed, a line shortened, a worktree read again and one that could
//! not be read. The project's runner program does not exist, so no agent is at work. Needs `git`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed is shared with its own tests, which read what this binary does not"
)]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

use std::{fs, path::PathBuf};

use kontur::{SeedRequest, scenarios};
use lurq::{
  app::{App, events::MouseButton, theme::PaletteColor},
  layout::text_style::FontWeight,
  node::color::Color,
};
use orchester_app::{
  routes::{ChangeSource, ChangedFile, ProjectPage, Route, TaskView},
  theme::{self, CodeText, ExtraColor, Mode, token_color},
};
use orchester_core::{
  id::ProjectId,
  integration::WORKTREES_DIR,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::highlight::TokenKind;
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
fn changes_lists_both_groups_with_counts_line_numbers_and_token_colors() {
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
  // A file in both groups counts once; KON-12 lists 6 distinct files.
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
  // The first file shows: an added migration, its lines numbered on the new side only.
  let line = page.texts("changes-diff-line");
  assert_eq!(line[..2], ["1", "+"], "no old number, new number 1: {line:?}");
  page.window.until("highlighted keywords", |window| {
    !window.tree.get_elements_by_class_name("code-keyword").is_empty()
  });
  for (kind, color) in [
    (TokenKind::Keyword, ExtraColor::CodeKeyword.role()),
    (TokenKind::String, ExtraColor::CodeString.role()),
    (TokenKind::Constant, ExtraColor::CodeNumber.role()),
    (TokenKind::Comment, PaletteColor::TextMuted),
    (TokenKind::Plain, PaletteColor::TextPrimary),
    (TokenKind::Function, PaletteColor::TextPrimary),
  ] {
    assert_eq!(token_color(kind), color, "{kind:?}");
  }
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
}

#[test]
fn changes_states_no_checkpoint_too_large_removed_and_line_shortened() {
  let mut page = Page::open();
  page.changes("KON-16", "changes-no-checkpoint");
  assert_eq!(page.tab_label(), "Changes", "no count before the first checkpoint");
  assert!(page.texts("changes-toolbar").contains(&"no checkpoint yet".to_owned()));

  page.changes("KON-25", "changes-too-large");
  let too_large = page.texts("changes-too-large").concat();
  assert!(too_large.contains("Too large to show here"), "{too_large}");
  assert!(too_large.contains("20,001"), "measured, not shown: {too_large}");
  assert!(!page.shows("changes-files"));
  assert_eq!(page.tab_label(), "Changes 1", "the files measured");

  page.changes("KON-26", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("too large to show"), "{notice}");
  assert!(
    page.shows("changes-group-checkpointed"),
    "the checkpointed files still show"
  );
  assert_eq!(page.tab_label(), "Changes");

  page.changes("KON-1", "changes-notice");
  assert!(page.texts("changes-notice").concat().contains("worktree was removed"));
  page.wait_for("changes-group-checkpointed");
  assert!(!page.shows("changes-group-worktree"));

  let file = ChangedFile::new("src/webhooks/format.rs", ChangeSource::Worktree).expect("a path");
  let key = "KON-27".parse().expect("a task key");
  let page_route = ProjectPage::Task(key, TaskView::Changes(Some(file)));
  page.window.navigation().go(Route::Project(page.project, page_route));
  page.wait_for("changes-line-cut");
  assert_eq!(page.texts("changes-line-cut"), ["Line shortened"]);
  assert!(
    !page.shows("changes-group-checkpointed"),
    "no checkpoint: only the worktree's group"
  );
}

#[test]
fn changes_reads_the_worktree_again_and_says_when_it_cannot() {
  let mut page = Page::open();
  page.changes("KON-12", "changes-group-worktree");
  page.wait_for("changes-worktree-file:tests/webhooks/jitter.rs");
  let worktree = page.worktree("KON-12");
  fs::write(worktree.join("notes.md"), "draft\n").expect("write into the worktree");
  // Leaving the tab and coming back reads the worktree again.
  page.go("KON-12/summary");
  page.wait_for("screen-task");
  page.changes("KON-12", "changes-worktree-file:notes.md");
  assert_eq!(page.texts("changes-group-worktree")[1], "3");
  page.window.until("the count of 7", |window| {
    window
      .texts("task-tab-changes-label")
      .is_some_and(|texts| texts.concat() == "Changes 7")
  });

  // A worktree git cannot read: a notice with Try again, the checkpointed files, and no count.
  fs::write(worktree.join(".git"), "gitdir: missing\n").expect("break the worktree's link");
  page.go("KON-12/summary");
  page.wait_for("screen-task");
  page.changes("KON-12", "changes-notice");
  let notice = page.texts("changes-notice").concat();
  assert!(notice.contains("Couldn't read what the worktree holds"), "{notice}");
  assert!(page.shows("changes-worktree-try-again"));
  assert!(page.shows("changes-group-checkpointed"));
  assert_eq!(page.tab_label(), "Changes");
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
