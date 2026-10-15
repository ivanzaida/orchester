//! The code viewer follows changes made outside it, on the seed's `changes` scenario (`examples/seed`), whose
//! KON-21 has a run recorded as running (given a worktree here): while an agent works in the run's worktree shown, the
//! tree and the file are read again on each change the runner reports of the project's runs and transcripts
//! (the feed's `changes`, which the test counts up as the runner would), keeping the selection and the scroll;
//! the checkout is not read again on those, only when the project's integration operations change. The
//! project's runner program does not exist, so nothing else reports. Needs `git`.

#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed is shared with its own tests, which read what this binary does not"
)]
mod kontur;
#[path = "../files/mcp_client.rs"]
mod mcp_client;
#[path = "../support/mod.rs"]
mod support;

use std::{
  fs,
  path::PathBuf,
  thread,
  time::{Duration, Instant},
};

use kontur::{SeedRequest, scenarios};
use mcp_client::Client;
use orchester_app::routes::{FilesView, ProjectPage, Route};
use orchester_core::{
  id::{ProjectId, RunId},
  integration::WORKTREES_DIR,
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::files::{LineRange, Version};
use serde_json::json;
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
    let launch = data.launch();
    Self {
      window: Window::of(data, launch),
      project: seeded.project,
      repo,
    }
  }

  /// KON-21's latest run, which the seed records as running, with a worktree of its own.
  fn working_run(&self) -> RunId {
    self.run_with_worktree("KON-21")
  }

  /// The latest run of the task `key`, with a worktree.
  fn run_with_worktree(&self, key: &str) -> RunId {
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
      .expect("a readable run")
      .id();
    let worktree = self.repo.join(WORKTREES_DIR).join(run.to_string());
    if !worktree.is_dir() {
      let worktree = worktree.to_string_lossy().into_owned();
      git(
        &self.repo,
        &["worktree", "add", "--quiet", "--detach", &worktree, "HEAD"],
      );
    }
    run
  }

  /// Opens `file` at `at` once the feed has read the project's tasks, which say whether an agent works.
  fn open_file(&mut self, file: &str, at: Version, lines: Option<LineRange>) {
    let feed = self.window.projects().feed(self.project);
    self
      .window
      .until("the project's tasks", move |_| feed.tasks.get().ready().is_some());
    let view = FilesView::new(Some(file), at).with_line(lines);
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Files(view)));
    self.window.until("the file", |window| {
      window.tree.get_element_by_id("files-code").is_some()
    });
  }

  fn meta(&mut self) -> String {
    self.window.text("files-meta-text")
  }

  /// The run `run` is cancelled, as when the person stops it: no agent works in its worktree any more. The
  /// project's tasks are read again, as after a save.
  fn cancel(&mut self, run: RunId) {
    let mut db = ProjectDb::open(&self.repo).expect("the project database opens");
    let mut record = db.run(run).expect("the run reads").expect("the run");
    let events = record.cancel(Timestamp::now()).expect("the run cancels");
    db.append_events(&record, &events).expect("the events are kept");
    self
      .window
      .projects()
      .feed(self.project)
      .revision
      .update(|count| *count += 1);
    self.window.pass();
  }

  /// The runner reports that a run or its transcript changed.
  fn run_changed(&mut self) {
    self
      .window
      .projects()
      .feed(self.project)
      .changes
      .update(|count| *count += 1);
    self.window.pass();
  }

  /// Types `text` key by key into the focused field, as a person does.
  fn type_keys(&mut self, text: &str) {
    for key in text.chars() {
      self
        .window
        .tree
        .key_down(key.to_string(), String::new(), key.is_uppercase(), false, false);
      self.window.pass();
    }
  }

  /// Lets the window deliver what its reads finish for a while.
  fn settle(&mut self) {
    for _ in 0..40 {
      self.window.tree.tick_futures();
      self.window.pass();
      thread::sleep(Duration::from_millis(10));
    }
  }
}

fn git(repo: &std::path::Path, args: &[&str]) {
  let output = std::process::Command::new("git")
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

/// `count` lines of text.
fn lines(count: usize) -> String {
  (1..=count).map(|line| format!("line {line}\n")).collect()
}

#[test]
fn files_follow_a_working_runs_worktree_and_keep_the_selection_and_the_scroll() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  fs::write(worktree.join("follow.md"), lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), LineRange::new(100, 101));
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });
  page.window.until("the range in view", |window| {
    window.tree.get_element_by_id("files-line-100").is_some()
  });

  fs::write(worktree.join("follow.md"), lines(125)).expect("the agent's edit");
  fs::write(worktree.join("added.md"), "new\n").expect("the agent's new file");
  page.run_changed();
  page.window.until("the file read again", |window| {
    window.text("files-meta-text").contains("125 lines")
  });
  page.window.until("the tree read again", |window| {
    window.tree.get_element_by_id("files-tree:added.md").is_some()
  });
  assert_eq!(page.window.text("files-selection"), "Lines 100–101 selected");
  assert!(
    page.window.tree.get_element_by_id("files-line-100").is_some(),
    "the scroll stayed at the selection"
  );
  assert!(
    page.window.tree.get_element_by_id("files-line-1").is_none(),
    "the lines did not scroll back to the top"
  );
}

#[test]
fn files_read_the_checkout_again_on_integration_and_never_on_run_changes() {
  let mut page = Page::open();
  fs::write(page.repo.join("follow.md"), lines(10)).expect("a file in the checkout");
  page.open_file("follow.md", Version::Checkout, None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("10 lines")
  });

  fs::write(page.repo.join("follow.md"), lines(12)).expect("an edit in the checkout");
  page.run_changed();
  page.settle();
  assert!(
    page.meta().contains("10 lines"),
    "the checkout is not read again on a run's change"
  );

  page.window.projects().feed(page.project).jobs.count_outside_removal();
  page.window.until("the checkout read again", |window| {
    window.text("files-meta-text").contains("12 lines")
  });
}

#[test]
fn files_do_not_follow_a_run_whose_agent_waits_for_the_person() {
  let mut page = Page::open();
  // The seed's KON-12 waits for an approval: no agent works in its worktree.
  let run = page.run_with_worktree("KON-12");
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  fs::write(worktree.join("follow.md"), lines(10)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("10 lines")
  });
  fs::write(worktree.join("follow.md"), lines(12)).expect("an edit");
  page.run_changed();
  page.settle();
  assert!(page.meta().contains("10 lines"), "not read again while no agent works");
}

#[test]
fn files_keep_the_current_place_found_when_the_file_is_read_again() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  fs::write(worktree.join("follow.md"), lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });
  page.window.press("files-find-open");
  page.type_keys("line");
  page.window.until("the places found", |window| {
    window.text("files-find-field-box").contains("1 of 120")
  });
  for _ in 0..2 {
    page.window.press("files-find-next");
  }
  assert!(page.window.text("files-find-field-box").contains("3 of 120"));

  fs::write(worktree.join("follow.md"), lines(125)).expect("the agent's edit");
  page.run_changed();
  page.window.until("the file read again", |window| {
    window.text("files-find-field-box").contains("of 125")
  });
  assert!(
    page.window.text("files-find-field-box").contains("3 of 125"),
    "the current place stays: {}",
    page.window.text("files-find-field-box")
  );
}

#[test]
fn files_keep_the_last_text_while_the_file_cannot_be_read_for_a_moment() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  let file = worktree.join("follow.md");
  fs::write(&file, lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });

  let held = hold(&file);
  page.run_changed();
  let read_again = Instant::now();
  // The coalesced wake, the read and its read once more fit in this.
  while read_again.elapsed() < Duration::from_millis(1_500) {
    page.window.tree.tick_futures();
    page.window.pass();
    thread::sleep(Duration::from_millis(10));
  }
  assert!(
    page.window.tree.get_element_by_id("files-code").is_some(),
    "the text stays while the file cannot be read"
  );
  assert!(page.meta().contains("120 lines"), "{}", page.meta());
  assert!(
    page.window.tree.get_element_by_id("files-stale").is_none(),
    "a moment's failure says nothing"
  );
  page.run_changed();
  page
    .window
    .until("the note that the text may be out of date", |window| {
      window.tree.get_element_by_id("files-stale").is_some()
    });
  assert!(
    page.window.text("files-stale").starts_with("May be out of date: "),
    "{}",
    page.window.text("files-stale")
  );
  assert!(page.meta().contains("120 lines"), "the text stays");
  drop(held);

  fs::write(&file, lines(125)).expect("the agent's edit");
  page.run_changed();
  page.window.until("the file read again", |window| {
    window.text("files-meta-text").contains("125 lines")
  });
  assert!(
    page.window.tree.get_element_by_id("files-stale").is_none(),
    "read, the note goes"
  );
}

#[test]
fn files_say_the_kept_text_may_be_out_of_date_once_following_ends() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  let file = worktree.join("follow.md");
  fs::write(&file, lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });
  let held = hold(&file);
  // An integration change reads the file again; ending the follow then changes nothing the reads wake on.
  page.window.projects().feed(page.project).jobs.count_outside_removal();
  let read_again = Instant::now();
  while read_again.elapsed() < Duration::from_millis(1_500) {
    page.window.tree.tick_futures();
    page.window.pass();
    thread::sleep(Duration::from_millis(10));
  }
  assert!(
    page.window.tree.get_element_by_id("files-stale").is_none(),
    "one failure while following says nothing yet"
  );
  page.cancel(run);
  page
    .window
    .until("the note once nothing reads the file again", |window| {
      window.tree.get_element_by_id("files-stale").is_some()
    });
  assert!(page.meta().contains("120 lines"), "the text stays");
  drop(held);
}

/// Holds `file` so that nothing else reads it until the result is dropped: open without sharing on Windows,
/// without read permission elsewhere.
#[cfg(windows)]
fn hold(file: &std::path::Path) -> fs::File {
  use std::os::windows::fs::OpenOptionsExt;
  fs::OpenOptions::new()
    .read(true)
    .share_mode(0)
    .open(file)
    .expect("the file held")
}

#[cfg(not(windows))]
fn hold(file: &std::path::Path) -> impl Drop {
  use std::os::unix::fs::PermissionsExt;
  struct Readable(PathBuf);
  impl Drop for Readable {
    fn drop(&mut self) {
      fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644)).expect("the file readable again");
    }
  }
  fs::set_permissions(file, fs::Permissions::from_mode(0o000)).expect("the file unreadable");
  Readable(file.to_path_buf())
}

#[test]
fn files_coalesce_the_runners_changes_and_keep_the_versions_until_integration_changes() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  fs::write(worktree.join("follow.md"), lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("its lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });
  let item = format!("files-version:{}", Version::Run(run));
  page.window.press("files-version");
  page.window.until("the versions read", |window| {
    window.tree.get_element_by_id(&item).is_some() && window.text(&item).contains("120 lines")
  });

  fs::write(worktree.join("follow.md"), lines(125)).expect("the agent's edit");
  let first = Instant::now();
  page.run_changed();
  page.window.until("the file read again", |window| {
    window.text("files-meta-text").contains("125 lines")
  });
  fs::write(worktree.join("follow.md"), lines(130)).expect("the agent's next edit");
  page.run_changed();
  // The reads woke at `first` at the earliest, so the next change waits until half a second after it.
  while first.elapsed() < Duration::from_millis(400) {
    page.window.tree.tick_futures();
    page.window.pass();
    assert!(page.meta().contains("125 lines"), "a change right after another waits");
    thread::sleep(Duration::from_millis(10));
  }
  page.window.until("the file read again after the wait", |window| {
    window.text("files-meta-text").contains("130 lines")
  });

  assert!(
    page.window.text(&item).contains("120 lines"),
    "the open menu's versions are not read again on a run's change: {}",
    page.window.text(&item)
  );
  page.window.projects().feed(page.project).jobs.count_outside_removal();
  page.window.until("the versions read again", |window| {
    window.text(&item).contains("130 lines")
  });
}

#[test]
fn files_read_a_working_runs_file_once_when_the_page_switches_to_it_and_once_per_change() {
  let mut page = Page::open();
  let run = page.working_run();
  let worktree = page.repo.join(WORKTREES_DIR).join(run.to_string());
  fs::write(page.repo.join("follow.md"), lines(10)).expect("a file in the checkout");
  fs::write(worktree.join("follow.md"), lines(120)).expect("a file in the worktree");
  page.open_file("follow.md", Version::Checkout, None);
  page.window.until("the checkout's lines", |window| {
    window.text("files-meta-text").contains("10 lines")
  });
  page.open_file("follow.md", Version::Run(run), None);
  page.window.until("the run's lines", |window| {
    window.text("files-meta-text").contains("120 lines")
  });
  page.settle();
  let client = Client::new(&mut page.window);
  let reads = |page: &mut Page| client.call(&mut page.window, "lurq_find_by_id", &json!({"id": "screen-files"}));
  let screen = reads(&mut page);
  assert!(
    screen.contains("{reads=2}"),
    "the checkout's read and the run's: {screen}"
  );

  fs::write(worktree.join("follow.md"), lines(125)).expect("the agent's edit");
  page.run_changed();
  page.window.until("the file read again", |window| {
    window.text("files-meta-text").contains("125 lines")
  });
  let screen = reads(&mut page);
  assert!(screen.contains("{reads=3}"), "one more on a change: {screen}");
}
