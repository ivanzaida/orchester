//! A fixture repository for the codebase index: a Cargo workspace with one crate, an npm package
//! with TypeScript and JavaScript, Python and Go files, Markdown documents, a binary file, and files
//! that are ignored or untracked.

#![allow(dead_code, reason = "each test binary uses a part of the fixture")]

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use orchester_core::{id::RunId, revision::Revision};
use orchester_index::{CodebaseIndexes, Ensured};
use tempfile::TempDir;

pub const LIB_RS: &str = r#"//! The fixture crate.

pub const LIMIT: usize = 3;

static NAME: &str = "app";

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Store {
  items: Vec<String>,
}

pub enum Error {
  Missing,
}

pub trait Named {
  fn name(&self) -> &str;

  fn shout(&self) -> String {
    self.name().to_uppercase()
  }
}

impl Store {
  pub fn new() -> Self {
    Self { items: Vec::new() }
  }

  fn helper(
    &self,
    index: usize,
  ) -> Option<&String> {
    fn inner() {}
    self.items.get(index)
  }
}

impl Named for Store {
  fn name(&self) -> &str {
    NAME
  }
}

macro_rules! twice {
  ($value:expr) => {
    $value * 2
  };
}

pub mod nested {
  pub fn deep() -> usize {
    twice!(LIMIT)
  }
}
"#;

pub const INDEX_TS: &str = r#"export interface Shape {
  area(): number;
  name: string;
}

export type Id = string;

export enum Color {
  Red,
  Blue,
}

export const LIMIT = 10;

export const double = (value: number): number => value * 2;

export class Circle implements Shape {
  constructor(private radius: number) {}

  area(): number {
    return Math.PI * this.radius ** 2;
  }

  get name(): string {
    return "circle";
  }
}

export function describe(shape: Shape): string {
  function local() {}
  return shape.name;
}

namespace Geometry {
  export function origin() {
    return 0;
  }
}
"#;

pub const LEGACY_JS: &str = "class Widget {\n  render() {\n    return null;\n  }\n}\n\nfunction mount(widget) {\n  return widget.render();\n}\n\nmodule.exports = { Widget, mount };\n";

pub const APP_PY: &str =
  "class Service:\n    def run(self):\n        return 1\n\n\n@decorated\ndef helper():\n    pass\n";

pub const MAIN_GO: &str = "package main\n\ntype Server struct {\n\tport int\n}\n\ntype Handler interface {\n\tServe()\n}\n\nconst Port = 8080\n\nfunc (s *Server) Start() error {\n\treturn nil\n}\n\nfunc main() {}\n";

pub const GUIDE_MD: &str = "---\ntitle: Guide\n---\n# Guide\n\nIntro.\n\n## Setup\n\n```sh\n# not a heading\n```\n\nUsage\n-----\n\n- item\n---\n\n### Details ###\n";

/// A repository with the fixture committed on `main`, and `.orchester/` excluded.
pub struct Repo {
  _dir: TempDir,
  pub root: PathBuf,
}

impl Repo {
  pub fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary directory");
    let root = dir.path().to_path_buf();
    let repo = Self { _dir: dir, root };
    repo.git(&["init", "--quiet", "--initial-branch=main"]);
    repo.git(&["config", "user.name", "Orchester Test"]);
    repo.git(&["config", "user.email", "test@orchester.invalid"]);
    repo.git(&["config", "core.autocrlf", "false"]);
    repo.git(&["config", "commit.gpgsign", "false"]);
    repo.write(".gitignore", "target/\nsecret.env\n");
    repo.write(
      "Cargo.toml",
      "[workspace]\nmembers = [\"crates/app\"]\n\n[workspace.package]\ndescription = \"The fixture workspace.\"\n",
    );
    repo.write(
      "crates/app/Cargo.toml",
      "[package]\nname = \"app\"\ndescription.workspace = true\n",
    );
    repo.write("crates/app/src/lib.rs", LIB_RS);
    repo.write("crates/app/src/main.rs", "fn main() {}\n");
    repo.write("crates/app/tests/it.rs", "#[test]\nfn works() {}\n");
    repo.write(
      "web/package.json",
      r#"{ "name": "web", "description": "The fixture web package.", "main": "./src/index.ts" }"#,
    );
    repo.write("web/src/index.ts", INDEX_TS);
    repo.write("web/src/legacy.js", LEGACY_JS);
    repo.write("tools/app.py", APP_PY);
    repo.write("svc/go.mod", "module example.com/svc\n\ngo 1.22\n");
    repo.write("svc/main.go", MAIN_GO);
    repo.write("docs/guide.md", GUIDE_MD);
    repo.write("README.md", "# Fixture\n");
    fs::write(repo.root.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1, 2]).expect("write binary");
    repo.git(&["add", "--all"]);
    repo.git(&["commit", "--quiet", "-m", "initial"]);
    repo.write("target/debug.rs", "fn ignored() {}\n");
    repo.write("secret.env", "TOKEN=hidden\n");
    repo.write("untracked.rs", "fn untracked() {}\n");
    fs::create_dir_all(repo.root.join(".orchester")).expect("create .orchester");
    fs::write(repo.root.join(".git/info/exclude"), ".orchester/\n").expect("exclude .orchester");
    repo
  }

  pub fn git(&self, args: &[&str]) -> String {
    let output = Command::new("git")
      .arg("-C")
      .arg(&self.root)
      .args(args)
      .output()
      .expect("run git");
    assert!(
      output.status.success(),
      "git {args:?} failed: {}",
      String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
  }

  pub fn write(&self, path: &str, content: &str) {
    let file = self.root.join(path);
    fs::create_dir_all(file.parent().expect("a file has a folder")).expect("create folder");
    fs::write(file, content).expect("write file");
  }

  pub fn head(&self) -> Revision {
    Revision::new(&self.git(&["rev-parse", "HEAD"])).expect("a commit id")
  }

  pub fn indexes(&self) -> CodebaseIndexes {
    CodebaseIndexes::new(&self.root)
  }

  /// A run with a worktree folder, as the runner creates it.
  pub fn run_with_worktree(&self) -> RunId {
    let run = RunId::generate();
    fs::create_dir_all(self.worktree(run)).expect("create worktree folder");
    run
  }

  pub fn worktree(&self, run: RunId) -> PathBuf {
    self.root.join(".orchester/worktrees").join(run.to_string())
  }

  /// The index of `revision` for a new run with a worktree.
  pub fn ensure(&self, revision: &Revision) -> Ensured {
    let run = self.run_with_worktree();
    self.indexes().ensure(revision, run).expect("index built")
  }

  pub fn index_root(&self) -> PathBuf {
    self.root.join(".orchester/index")
  }
}

pub fn read(dir: &Path, name: &str) -> String {
  fs::read_to_string(dir.join(name)).unwrap_or_else(|error| panic!("read {name}: {error}"))
}

/// Every file under `dir` but `manifest.json`, by relative path, with its content.
pub fn files(dir: &Path) -> Vec<(String, String)> {
  let mut found = Vec::new();
  collect(dir, dir, &mut found);
  found.sort();
  found
}

fn collect(root: &Path, dir: &Path, found: &mut Vec<(String, String)>) {
  for entry in fs::read_dir(dir).expect("read folder") {
    let path = entry.expect("folder entry").path();
    if path.is_dir() {
      collect(root, &path, found);
      continue;
    }
    let relative = path
      .strip_prefix(root)
      .expect("inside the index")
      .to_string_lossy()
      .replace('\\', "/");
    if relative != "manifest.json" {
      found.push((relative, fs::read_to_string(&path).expect("read index file")));
    }
  }
}
