//! Command lines whose every part runs on its own run without a person (`run::permitted`): a leading
//! `cd` into the worktree, literal variables, chains and pipes of known-safe reads and listed commands.
//! Writes through the shell, redirections, background parts and what the parser cannot read still ask.
//! Sources in Cargo's registry and the toolchains are read outside the worktree, not credentials.
//! `cargo test -p orchester-core --test command_sequences command_sequences`.

use std::path::{Path, PathBuf};

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{ApprovalSubject, RuleScope, SimilarUnavailable, permitted},
};
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  home: String,
  worktree: PathBuf,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = root.path().join("repo").join(".orchester").join("worktrees").join("w1");
    let lurq = registry(&home).join("lurq-0.40.1").join("src");
    for folder in [
      &worktree.join("src"),
      &lurq,
      &home.join(".ssh"),
      &root.path().join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")));
    Self {
      home: slashed(&home),
      worktree,
      scope,
      _root: root,
    }
  }

  /// `line` with `{HOME}` and the worktree's forms: `{WT}` as the host writes it (quoted where it has
  /// backslashes), `{WT/}` with forward slashes and `{WTBASH}` as Git Bash writes it on Windows (`/c/…`).
  fn command(&self, line: &str) -> ClassifiedCommand {
    let line = line
      .replace("{HOME}", &self.home)
      .replace("{WTBASH}", &bash_form(&self.worktree))
      .replace("{WT/}", &slashed(&self.worktree))
      .replace("{WT}", &self.worktree.to_string_lossy());
    ClassifiedCommand::classify(&line, &self.scope)
  }

  fn permitted(&self, line: &str) -> Option<CommandCategory> {
    permitted(&ApprovalSubject::Command(self.command(line)), &reading(builder()))
  }
}

fn registry(home: &Path) -> PathBuf {
  home
    .join(".cargo")
    .join("registry")
    .join("src")
    .join("index.crates.io-1949cf8c6b5b557f")
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// `path` as Git Bash names it on Windows (`C:\x` is `/c/x`); elsewhere as it is.
fn bash_form(path: &Path) -> String {
  let text = slashed(path);
  match text.as_bytes() {
    [drive, b':', ..] if cfg!(windows) => format!("/{}{}", char::from(drive.to_ascii_lowercase()), &text[2..]),
    _ => text,
  }
}

fn allowed(entries: &[&str]) -> CommandPermission {
  let entries = entries
    .iter()
    .map(|entry| CommandPrefix::new(entry).expect("prefix"))
    .collect();
  CommandPermission::Allowed(AllowedCommands::new(entries).expect("allowed commands"))
}

/// The permissions of a read-only step with `commands`: every write asks.
fn reading(commands: CommandPermission) -> AgentPermissions {
  AgentPermissions::new(RepositoryAccess::ReadOnly, commands, NetworkAccess::Off)
}

/// A builder's list: the checks it runs.
fn builder() -> CommandPermission {
  allowed(&["cargo test", "cargo check", "cargo clippy"])
}

#[test]
fn command_sequences_of_parts_that_run_alone_run() {
  let fixture = Fixture::new();
  let cases = [
    ("cd {WTBASH} && cargo test -p orchester-core", CommandCategory::Worktree),
    ("cd {WT/}; git status && git diff --stat", CommandCategory::ReadOnly),
    ("cd '{WT}' && git log --oneline -3", CommandCategory::ReadOnly),
    (
      "cd \"{WT}\"; cargo check 2>&1 | grep -E 'error' -A6 | head -4",
      CommandCategory::Worktree,
    ),
    (
      "f=src/lib.rs; sed -n 40,52p $f; grep -n fn \"$f\" | wc -l",
      CommandCategory::ReadOnly,
    ),
    ("cd {WTBASH}; L=src; sed -n 1p ${L}/lib.rs", CommandCategory::ReadOnly),
    (
      "grep -rn 'Finished {' src --include=*.rs | awk -F: '{print $1}' | sort | uniq -c",
      CommandCategory::ReadOnly,
    ),
    ("sed -n '/fn main/,/^}/p' src/lib.rs", CommandCategory::ReadOnly),
    ("sed -n '1,20{p;q}' src/lib.rs", CommandCategory::ReadOnly),
    ("find src -name '*.rs' | wc -l", CommandCategory::ReadOnly),
    (
      "echo finished; cat src/lib.rs 2>/dev/null | tail -n 5",
      CommandCategory::ReadOnly,
    ),
    ("rg -n Approval src || true", CommandCategory::ReadOnly),
    ("f=src && ls $f", CommandCategory::ReadOnly),
    ("awk 'NR>=129 && NR<=160' src/lib.rs", CommandCategory::ReadOnly),
  ];
  for (line, expected) in cases {
    assert_eq!(fixture.permitted(line), Some(expected), "`{line}` runs");
  }
}

#[test]
fn command_sequences_with_a_part_that_would_ask_alone_ask() {
  let fixture = Fixture::new();
  let asks = [
    // Writes through the shell, which the edit tools do without asking.
    "cd {WTBASH}; f=src/lib.rs; sed -i 's/a/b/' $f; sed -n 1,5p $f",
    "sed -i.bak 1d src/lib.rs",
    "sed -ni 1p src/lib.rs",
    "sed --in 1p src/lib.rs",
    "sed -n '1w out.txt' src/lib.rs",
    "sed -n 1e src/lib.rs",
    "sed -e 1p src/lib.rs",
    "sed -n 1p src/lib.rs > out.txt",
    "cat src/lib.rs >> notes.txt",
    "cat src/lib.rs | tee out.txt",
    "cat > src/x.rs <<'EOF'\nfn x() {}\nEOF",
    "awk '{system(\"rm -rf src\")}' src/lib.rs",
    "awk '{print > \"out.txt\"}' src/lib.rs",
    "awk '{\"date\" | getline d}' src/lib.rs",
    "awk 'BEGIN{print ENVIRON[\"TOKEN\"]}'",
    "awk -f prog.awk src/lib.rs",
    "find src -delete",
    "find src -exec cat src/lib.rs ';'",
    "sort -o out.txt src/lib.rs",
    "sort --out=out.txt src/lib.rs",
    "sort --compress-program=sh src/lib.rs",
    // Background, subshells, substitutions and variables not set to a literal.
    "cargo test &",
    "cargo test & git status",
    "(cd src && ls)",
    "cat $(git ls-files)",
    "f=$(pwd); cat $f",
    "f=src; f=$(pwd); ls $f",
    "\"f=src\"; ls $f",
    "'f'=src; ls $f",
    "f='a b'; cat $f",
    "false && f=src; ls $f",
    "f=src | true; ls $f",
    "PATH=/tmp; cargo test",
    "http_proxy=x; cargo test",
    "CC=x; cargo test",
    "cat $HOME/x",
    // Outside the worktree, credentials, installs, unlisted programs.
    "cd {WTBASH}/..; ls",
    "cd /; ls",
    "cat ~/.ssh/id_rsa",
    "f=~/.ssh/id_rsa; cat $f",
    "cat {HOME}/.cargo/credentials.toml",
    "sed -n 1p {HOME}/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.40.1/src/lib.rs",
    "npm install && git status",
    "cargo metadata && git status",
    "python3 build.py",
    "git status; git push",
  ];
  for line in asks {
    assert_eq!(fixture.permitted(line), None, "`{line}` must ask");
  }
  // With every command asking, or no listed command, nothing runs by itself.
  let subject = ApprovalSubject::Command(fixture.command("git status && git diff"));
  assert_eq!(permitted(&subject, &reading(CommandPermission::AskBefore)), None);
  assert_eq!(permitted(&subject, &reading(allowed(&[]))), None);
  assert_eq!(permitted(&subject, &reading(CommandPermission::AnyInWorktree)), None);
}

#[test]
fn command_sequences_offer_approve_similar_for_the_part_that_asks() {
  let fixture = Fixture::new();
  let similar = |line: &str| RuleScope::similar(&ApprovalSubject::Command(fixture.command(line)));
  let Ok(RuleScope::SimilarCommand { command }) =
    similar("cd {WTBASH}; f=src/lib.rs; sed -i 's/a/b/' $f; sed -n 1,5p $f")
  else {
    panic!("a sed edit in the worktree can be approved similarly");
  };
  assert_eq!(
    (command.words(), command.category()),
    ("sed".to_owned(), CommandCategory::Worktree)
  );
  // The leading `cd` is not a part of its own.
  let parts = fixture.command("cd {WT/} && cargo test").parts().len();
  assert_eq!(parts, 1);
  // Several parts that ask alike make one rule; unlike ones make none.
  assert!(similar("sed -i 1d src/a.rs; sed -i 2d src/b.rs").is_ok());
  assert_eq!(
    similar("sed -i 1d src/a.rs; mkdir -p out"),
    Err(SimilarUnavailable::SeveralAsk)
  );
  assert_eq!(
    similar("python3 - <<'EOF'\nprint(1)\nEOF"),
    Err(SimilarUnavailable::Unknown)
  );
  assert_eq!(similar("cat ~/.ssh/id_rsa"), Err(SimilarUnavailable::Secrets));
}

#[test]
fn command_sequences_read_published_sources_outside_and_keep_credentials() {
  let fixture = Fixture::new();
  let reads = [
    "sed -n 4330,4600p \"{HOME}/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.40.1/src/app/runtime.rs\"",
    "ls ~/.cargo/registry/src/*/ | rg lurq",
    "L=\"{HOME}/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.40.1/src\"; rg -n drag $L | head -40",
    "cd ~/.cargo/registry/src/*/lurq-0.40.1 && grep -rn interval src | head -50; ls docs 2>/dev/null",
    "cat ~/.rustup/toolchains/stable-x86_64-pc-windows-msvc/lib/rustlib/src/rust/library/core/src/lib.rs",
  ];
  for line in reads {
    let command = fixture.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "`{line}`");
    assert!(!command.reads_secrets(), "`{line}` reads no credential");
    assert!(
      RuleScope::similar(&ApprovalSubject::Command(command)).is_ok(),
      "`{line}` can be approved similarly"
    );
  }
  let secrets = [
    "cat ~/.cargo/credentials.toml",
    "cat ~/.cargo/credentials",
    "cat ~/.cargo/config.toml",
    "ls ~/.cargo/registry",
    "cat ~/.cargo/registry/index/x",
    "cat ~/.rustup/settings.toml",
    "ls ~/.rustup/toolchains",
    "cat ~/.ssh/id_rsa",
  ];
  for line in secrets {
    assert!(fixture.command(line).reads_secrets(), "`{line}` reads a credential");
  }
  // What a glob matched, left with `..`, cannot be told.
  let climbing = fixture.command("cat ~/.cargo/registry/src/*/../../../credentials.toml");
  assert_eq!(climbing.category(), CommandCategory::Unknown);
  // A `cd` whose glob matches no folder, or several, leaves where later parts run unknown.
  let unmatched = fixture.command("cd ~/.cargo/registry/src/none*/lurq && cat ../x.rs");
  assert_eq!(unmatched.category(), CommandCategory::Unknown);
}
