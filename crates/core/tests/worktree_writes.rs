//! Shell writes in the worktree (the user's decision of 2026-10-05): in a step that edits files, a
//! command line whose only effects are writes to files in the worktree or a writable folder, with parts
//! that run on their own, runs without a person, as the edit tools write there without asking. A write
//! into `.git` or `.orchester/`, through a link out, outside, a removal of the worktree itself, an
//! expanding here-document or a read outside still asks; in a read-only step every write asks.
//! `cargo test -p orchester-core --test worktree_writes worktree_writes`.

use std::path::Path;

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandScope},
  },
  run::{ApprovalSubject, permitted},
};
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  home: String,
  worktree: String,
  build: String,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = root.path().join("repo").join("w1");
    let build = root.path().join("build");
    let elsewhere = root.path().join("elsewhere");
    for folder in [
      &worktree.join("src"),
      &worktree.join(".git").join("hooks"),
      &worktree.join(".orchester"),
      &home.join(".ssh"),
      &build,
      &elsewhere,
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src").join("lib.rs"), "fn x() {}\n").expect("source");
    std::fs::write(worktree.join(".git").join("config"), "[core]\n").expect("config");
    std::fs::write(home.join(".ssh").join("id_rsa"), "key").expect("key");
    std::fs::write(home.join(".bashrc"), "").expect("bashrc");
    link_folder(&worktree.join("out"), &elsewhere);
    std::fs::create_dir_all(worktree.join("src").join("venv")).expect("venv");
    link_folder(&worktree.join("src").join("venv").join("link"), &elsewhere);
    let scope = CommandScope::new(
      &worktree,
      std::slice::from_ref(&build),
      Some(&home),
      Some(&root.path().join("temp")),
    );
    Self {
      home: slashed(&home),
      worktree: slashed(&worktree),
      build: slashed(&build),
      scope,
      _root: root,
    }
  }

  fn runs(&self, line: &str, repository: RepositoryAccess) -> bool {
    let line = line
      .replace("{HOME}", &self.home)
      .replace("{WT}", &self.worktree)
      .replace("{BUILD}", &self.build);
    let command = ClassifiedCommand::classify(&line, &self.scope);
    let entries = ["cargo test", "cargo check", "cargo fmt"]
      .iter()
      .map(|entry| CommandPrefix::new(entry).expect("prefix"))
      .collect();
    let list = CommandPermission::Allowed(AllowedCommands::new(entries).expect("list"));
    let permissions = AgentPermissions::new(repository, list, NetworkAccess::Off);
    permitted(&ApprovalSubject::Command(command), &permissions).is_some()
  }
}

/// A link to the folder `target` at `link`: a symbolic link on Unix, a junction on Windows, which needs
/// no privilege there and which Windows follows like a link.
fn link_folder(link: &Path, target: &Path) {
  #[cfg(unix)]
  std::os::unix::fs::symlink(target, link).expect("link");
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link)
      .arg(target)
      .output()
      .expect("mklink starts");
    assert!(made.status.success(), "junction: {made:?}");
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// The request that asked a person before: a `cd` into the worktree, a variable, a `sed -i`, a quoted
/// here-document appended to the file, then a listed test piped into filters, on lines of their own.
const REAL_REQUEST: &str = "cd {WT}; f=src/lib.rs; sed -i 's|a|b|; s|^c$|d|' $f; cat >> $f <<'EOF'
#[test]
fn native_setup_reads_$(this)() {
  assert!(`true`);
}
EOF
cargo test -p orchester-core --test native_setup native_setup 2>&1 | grep -E \"test result|panicked|^error\" -A6 | head -30";

/// Lines that only write in the worktree or a writable folder.
const EDITS: &[&str] = &[
  REAL_REQUEST,
  "sed -i 's/a/b/' src/lib.rs",
  "echo x > src/new.rs",
  "printf 'x' >> src/lib.rs",
  "cat > src/new.rs <<'EOF'\nfn $(x)() {}\nEOF",
  "cat >> src/lib.rs <<EOF\nplain text\nEOF",
  "cat <<-\"EOF\" > src/new.rs\n\tfn x() {}\n\tEOF",
  "echo x | tee -a src/lib.rs",
  "tee src/a.rs < src/lib.rs",
  "mkdir -p src/deep/er\ntouch src/x.rs",
  "mkdir -p src; echo x > src/made.rs",
  "cp src/lib.rs src/copy.rs\nmv src/copy.rs src/moved.rs",
  "rm src/moved.rs; rm -rf src/deep",
  "echo x > {BUILD}/out.txt",
  "git status\necho done > src/log.txt",
];

/// Lines a step that edits files still asks about.
const ASKS: &[&str] = &[
  "sed -i s/a/b/ .git/config",
  "sed -i s/a/b/ .git/hooks/pre-commit",
  "sed -i s/a/b/ ../outside.txt",
  "sed -i s/a/b/ out/x.txt",
  "sed -i s/a/b/ /etc/x",
  "sed -i s/a/b/ ~/.bashrc",
  "sed -i s/a/b/ {HOME}/.bashrc",
  "sed -i '1w src/x' src/lib.rs",
  "sed -i 1e src/lib.rs",
  "cat > src/x.rs <<EOF\n$(rm -rf ~)\nEOF",
  "cat > src/x.rs <<EOF\n`id`\nEOF",
  "cat > src/x.rs <<EOF\n$HOME\nEOF",
  "cat > src/x.rs <<'EOF'\nfn x() {}",
  "cat > src/x.rs <<< text",
  "echo x > .git/HEAD",
  "echo x > .orchester/x",
  "touch .git/hooks/pre-commit",
  "mkdir -p .git/hooks/x",
  "cp src/lib.rs .git/hooks/pre-commit",
  "rm -rf .",
  "rm -rf /",
  "rm -rf src/..",
  "rm -rf {WT}",
  "rm -rf {BUILD}",
  "cp {HOME}/.ssh/id_rsa src/",
  "cp ../elsewhere.txt src/",
  "cp out/x src/",
  "mv src/lib.rs ../",
  "mv src/lib.rs /tmp/x",
  "tee ../x < src/lib.rs",
  "echo x > out/x",
  "chmod +x src/lib.rs",
  "ln -s ../.. src/up",
  "python3 - <<'EOF'\nprint(1)\nEOF",
  // A tree or a link copied or moved can bring links that lead anywhere.
  "cp -r src src2",
  "cp -R src src2",
  "cp -a src/lib.rs src/x.rs",
  "cp -P src/lib.rs src/x.rs",
  "cp -d src/lib.rs src/x.rs",
  "cp --recursive src src2",
  "cp out src/o",
  "cp src src2",
  "mv out src/o",
  "mv src src3",
  "cp -r src/venv src/v2 && echo x > src/v2/link/f",
  // A write under what an earlier part made is not placed by what is there now.
  "mkdir -p src/deep/er && touch src/deep/er/x.rs",
  "mkdir -p src/venv && echo x > src/venv/link/f",
  "touch src/t; echo x > src/t/f",
  "cp src/lib.rs src/a.rs && echo x > src/a.rs/f",
];

#[test]
fn worktree_writes_run_in_a_step_that_edits_files() {
  let fixture = Fixture::new();
  for line in EDITS {
    assert!(fixture.runs(line, RepositoryAccess::Write), "`{line}` runs");
  }
}

#[test]
fn worktree_writes_ask_where_no_edit_may_write() {
  let fixture = Fixture::new();
  for line in ASKS {
    assert!(!fixture.runs(line, RepositoryAccess::Write), "`{line}` must ask");
  }
}

/// Git Bash places a path rooted without a drive under its own installation, which is not placed.
#[test]
fn worktree_writes_ask_for_a_path_rooted_without_a_drive() {
  let fixture = Fixture::new();
  for line in [
    "echo x > /projects/x",
    "sed -i s/a/b/ /tmp/x",
    "cp src/lib.rs /projects/x",
  ] {
    assert!(!fixture.runs(line, RepositoryAccess::Write), "`{line}` must ask");
  }
}

#[test]
fn worktree_writes_ask_in_a_read_only_step() {
  let fixture = Fixture::new();
  for line in EDITS.iter().chain(ASKS) {
    assert!(!fixture.runs(line, RepositoryAccess::ReadOnly), "`{line}` must ask");
  }
}

#[test]
fn worktree_writes_take_cargo_toolchains_as_the_listed_command() {
  let fixture = Fixture::new();
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    assert!(fixture.runs("cargo +nightly fmt --all", repository));
    assert!(fixture.runs("cd {WT} && cargo +nightly fmt -p orchester-core", repository));
  }
  assert!(!fixture.runs("cargo +nightly", RepositoryAccess::Write));
  assert!(!fixture.runs("cargo +nightly build", RepositoryAccess::Write));
}
