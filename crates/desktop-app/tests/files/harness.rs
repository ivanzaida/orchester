//! Kontur as a real git repository with a few files committed on `main`, its window with the stand-ins (the
//! clipboard among them), and a way to wait for the code viewer's states.
use std::{
  fs,
  path::{Path, PathBuf},
  process,
};

use orchester_app::routes::{FilesView, ProjectPage, Route};
use orchester_workspace::files::Version;

use super::support::{Data, Window, project_id, stand_ins::StandIns};

/// Text in the committed `src/main.rs` that must never reach automation.
pub const MARKER: &str = "kontur_private_marker_7319";

pub struct Files {
  pub window: Window,
  pub stand_ins: StandIns,
  pub repo: PathBuf,
}

pub fn git(repo: &Path, args: &[&str]) -> String {
  let output = process::Command::new("git")
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
  String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

impl Files {
  pub fn new() -> Self {
    let data = Data::new();
    let repo = data.repo("repo-kontur");
    fs::remove_dir_all(repo.join(".git")).expect("the stand-in .git");
    git(&repo, &["init", "--quiet", "--initial-branch=main"]);
    git(&repo, &["config", "user.name", "Orchester Test"]);
    git(&repo, &["config", "user.email", "test@orchester.invalid"]);
    git(&repo, &["config", "core.autocrlf", "false"]);
    fs::create_dir_all(repo.join("src/ui")).expect("src");
    let main: String = (1..=120)
      .map(|line| match line {
        1 => format!("// {MARKER}\n"),
        2 => "fn main() {\n".to_owned(),
        119 => "}\n".to_owned(),
        _ => format!("  let value_{line} = {line}; // limit\n"),
      })
      .collect();
    fs::write(repo.join("src/main.rs"), main).expect("main.rs");
    fs::write(repo.join("src/ui/row.rs"), "pub struct Row;\n").expect("row.rs");
    fs::write(repo.join("README.md"), "# Kontur\n").expect("README");
    fs::write(repo.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1, 2, 3]).expect("logo");
    fs::write(repo.join(".gitignore"), ".orchester/\n").expect(".gitignore");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "--quiet", "-m", "first"]);
    let launch = data.launch();
    data.add_kontur(launch.workspace().expect("the session started"));
    drop(launch);
    let (window, stand_ins) = Window::with_stand_ins(data);
    Self {
      window,
      stand_ins,
      repo,
    }
  }

  /// Opens `file` at `at` and waits until the page shows the file, or what stands for it.
  pub fn open(&mut self, file: Option<&str>, at: Version) {
    let view = FilesView::new(file, at);
    self.go(view);
  }

  pub fn go(&mut self, view: FilesView) {
    self
      .window
      .navigation()
      .go(Route::Project(project_id(), ProjectPage::Files(view)));
    self.settle();
  }

  /// Waits until the page and its tree are read, and its file or what stands for it is shown.
  pub fn settle(&mut self) {
    self.window.until("the code viewer read", |w| {
      let shown = ["files-code", "files-state", "files-gone"]
        .iter()
        .any(|id| w.tree.get_element_by_id(id).is_some());
      let tree = w.tree.get_element_by_id("files-tree-loading").is_none();
      shown && tree
    });
  }

  /// The bounds of the element `id`.
  pub fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// Types `text` key by key into the focused field, as a person does.
  pub fn type_keys(&mut self, text: &str) {
    for key in text.chars() {
      let shift = key.is_uppercase();
      self
        .window
        .tree
        .key_down(key.to_string(), String::new(), shift, false, false);
      self.window.pass();
    }
  }

  /// The route the window's active tab shows.
  pub fn route(&self) -> Route {
    self.window.navigation().route_now()
  }
}
