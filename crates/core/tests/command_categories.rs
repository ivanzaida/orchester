use std::path::{Path, PathBuf};

use orchester_core::permission::category::{ClassifiedCommand, CommandCategory, CommandReason, CommandScope};
use tempfile::TempDir;

/// A home folder with a worktree in it, a writable build folder and a temporary folder beside them.
struct Folders {
  _root: TempDir,
  home: PathBuf,
  worktree: PathBuf,
  scope: CommandScope,
}

impl Folders {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("projects").join("kontur");
    let build = root.path().join("build");
    let temp = root.path().join("temp");
    for folder in [&worktree, &build, &temp, &home.join(".ssh")] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    let scope = CommandScope::new(&worktree, &[build], Some(&home), Some(&temp));
    Self {
      _root: root,
      home,
      worktree,
      scope,
    }
  }

  fn classify(&self, command: &str) -> ClassifiedCommand {
    ClassifiedCommand::classify(command, &self.scope)
  }

  fn category(&self, command: &str) -> CommandCategory {
    self.classify(command).category()
  }

  fn abs(&self, path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
  }
}

#[test]
fn command_categories_by_program_and_subcommand() {
  let folders = Folders::new();
  let cases = [
    ("git status", CommandCategory::ReadOnly),
    ("cat src/lib.rs | wc -l", CommandCategory::ReadOnly),
    ("rg -n Approval crates", CommandCategory::ReadOnly),
    ("git add -A && git commit -m 'sync worker'", CommandCategory::Worktree),
    ("cargo test -p orchester-core", CommandCategory::Worktree),
    ("sed -i s/a/b/ src/lib.rs", CommandCategory::Worktree),
    ("mkdir -p target/out", CommandCategory::Worktree),
    ("git fetch origin", CommandCategory::Network),
    ("cargo add serde --features derive", CommandCategory::Network),
    ("curl https://example.dev", CommandCategory::Network),
    ("git push origin main", CommandCategory::HistoryDestructive),
    ("git reset --hard HEAD~1", CommandCategory::HistoryDestructive),
    ("git branch -D old", CommandCategory::HistoryDestructive),
    ("git commit --amend --no-edit", CommandCategory::HistoryDestructive),
    ("./scripts/regen.sh", CommandCategory::Unknown),
    ("echo $HOME", CommandCategory::Unknown),
    ("cat $(git ls-files)", CommandCategory::Unknown),
    ("python build.py", CommandCategory::Unknown),
  ];
  for (command, expected) in cases {
    assert_eq!(folders.category(command), expected, "{command}");
  }
}

#[test]
fn command_categories_place_written_paths() {
  let folders = Folders::new();
  let outside = folders.abs(&folders.home.join("notes.txt"));
  let temp = folders.abs(&folders.home.parent().expect("root").join("temp").join("worker_orig.rs"));
  let build = folders.abs(&folders.home.parent().expect("root").join("build").join("out"));
  assert_eq!(
    folders.category(&format!("cp src/a.rs {outside}")),
    CommandCategory::OutsideWrite
  );
  assert_eq!(
    folders.category(&format!("cp src/a.rs {temp}")),
    CommandCategory::OutsideWrite
  );
  assert_eq!(folders.category(&format!("touch {build}")), CommandCategory::Worktree);
  assert_eq!(
    folders.category(&format!("echo hi > {outside}")),
    CommandCategory::OutsideWrite
  );
  assert_eq!(folders.category("echo hi > out.txt 2>&1"), CommandCategory::Worktree);
  assert_eq!(folders.category("cargo build 2> /dev/null"), CommandCategory::Worktree);
  assert_eq!(folders.category(&format!("rm {temp}")), CommandCategory::OutsideWrite);
  assert_eq!(
    folders.category(&format!("rm -rf {outside}")),
    CommandCategory::HistoryDestructive
  );
  assert_eq!(
    folders.category("rm -rf ../../other"),
    CommandCategory::HistoryDestructive
  );
  assert_eq!(folders.category("rm -rf build"), CommandCategory::Worktree);
  // A later part runs in the folder a `cd` before it went to.
  assert_eq!(
    folders.category("cd .. && rm notes"),
    CommandCategory::HistoryDestructive
  );
  assert_eq!(folders.category("cd src && rm old.rs"), CommandCategory::Worktree);
}

#[test]
fn command_categories_strictest_part_names_the_command() {
  let folders = Folders::new();
  let command = folders.classify("git status && cargo add serde && git log -1");
  assert_eq!(command.parts().len(), 3);
  assert_eq!(command.category(), CommandCategory::Network);
  let forcing = command.forcing().expect("forcing part");
  assert_eq!(forcing.words()[..2], ["cargo".to_owned(), "add".to_owned()]);
  assert_eq!(command.asking().count(), 1);
  let tie = folders.classify("git push && ./run.sh");
  assert_eq!(tie.category(), CommandCategory::HistoryDestructive);
}

#[test]
fn command_categories_follow_shell_scripts() {
  let folders = Folders::new();
  assert_eq!(
    folders.category("bash -lc 'git status && git diff'"),
    CommandCategory::ReadOnly
  );
  assert_eq!(
    folders.category(r#""C:\Program Files\PowerShell\7\pwsh.exe" -Command "Get-ChildItem src""#),
    CommandCategory::ReadOnly
  );
  assert_eq!(
    folders.category("pwsh -NoProfile -Command git push"),
    CommandCategory::HistoryDestructive
  );
  assert_eq!(folders.category("pwsh -File build.ps1"), CommandCategory::Unknown);
  assert_eq!(folders.category("bash script.sh"), CommandCategory::Unknown);
}

#[test]
fn command_categories_note_secrets_and_installs() {
  let folders = Folders::new();
  assert!(folders.classify("cat ~/.ssh/id_rsa").reads_secrets());
  assert!(folders.classify("ls ~/.codex").reads_secrets());
  assert!(folders.classify("cat ~/.claude.json").reads_secrets());
  assert!(folders.classify("security find-generic-password -s x").reads_secrets());
  assert!(!folders.classify("cat src/.env.example").reads_secrets());
  let worktree_dot = folders.abs(&folders.worktree.join(".github"));
  assert!(!folders.classify(&format!("ls {worktree_dot}")).reads_secrets());
  for command in [
    "npm install left-pad",
    "pnpm add -D vitest",
    "pip install requests",
    "cargo install ripgrep",
    "cargo add serde",
    "npx create-thing",
    "brew install jq",
    "npm run build -g",
  ] {
    assert!(folders.classify(command).installs(), "{command}");
  }
  for command in ["npm run build", "cargo test", "git status"] {
    assert!(!folders.classify(command).installs(), "{command}");
  }
}

#[test]
fn command_categories_similar_and_prefixes() {
  let folders = Folders::new();
  let first = folders.classify("cargo add serde --features derive");
  let later = folders.classify("cargo add tokio");
  let other = folders.classify("cargo test");
  let rule = first.forcing().and_then(|part| part.similar()).expect("similar rule");
  assert_eq!(rule.words(), "cargo add");
  assert_eq!(rule.category(), CommandCategory::Network);
  assert!(later.forcing().is_some_and(|part| part.is_similar(&rule)));
  assert!(!other.forcing().is_some_and(|part| part.is_similar(&rule)));
  let prefixes: Vec<String> = first
    .forcing()
    .expect("part")
    .prefixes()
    .iter()
    .map(ToString::to_string)
    .collect();
  assert_eq!(prefixes, ["cargo add", "cargo add serde --features derive"]);
  assert!(!first.forcing().expect("part").has_wider_forms());
  assert!(
    folders
      .classify("git reset src/a.rs")
      .forcing()
      .expect("part")
      .has_wider_forms()
  );
  assert!(folders.classify("./x.sh").forcing().expect("part").similar().is_none());
}

#[test]
fn command_categories_reason_names_what_asks() {
  let folders = Folders::new();
  let outside = folders.abs(&folders.home.join("notes.txt"));
  let command = |words: &str| CommandReason::Network {
    command: words.to_owned(),
  };
  let cases = [
    (
      format!("cp src/a.rs {outside}"),
      CommandReason::WritesOutside { path: outside },
    ),
    (
      "rm -rf ../../other".to_owned(),
      CommandReason::DeletesOutside {
        path: "../../other".to_owned(),
      },
    ),
    (
      "git push origin main".to_owned(),
      CommandReason::RewritesHistory {
        command: "git push".to_owned(),
      },
    ),
    (
      "cat ~/.ssh/id_rsa".to_owned(),
      CommandReason::ReadsCredentials {
        what: "~/.ssh/id_rsa".to_owned(),
      },
    ),
    ("git fetch origin".to_owned(), command("git fetch")),
    ("./scripts/regen.sh".to_owned(), CommandReason::Unknown),
    (
      "cargo test -p core".to_owned(),
      CommandReason::Worktree {
        command: "cargo test".to_owned(),
      },
    ),
    (
      "git status".to_owned(),
      CommandReason::ReadOnly {
        command: "git status".to_owned(),
      },
    ),
  ];
  for (line, reason) in cases {
    assert_eq!(folders.classify(&line).reason(), Some(reason), "{line}");
  }
}

#[test]
fn command_categories_reason_names_an_install_source() {
  let folders = Folders::new();
  let cases = [
    (
      "cargo add serde --features derive".to_owned(),
      CommandReason::Installs {
        registry: "crates.io".to_owned(),
        package: Some("serde".to_owned()),
        listed_in: None,
      },
    ),
    (
      "cargo add --features derive serde".to_owned(),
      CommandReason::Installs {
        registry: "crates.io".to_owned(),
        package: Some("serde".to_owned()),
        listed_in: None,
      },
    ),
    (
      "pip install -r requirements.txt".to_owned(),
      CommandReason::Installs {
        registry: "PyPI".to_owned(),
        package: None,
        listed_in: Some("requirements.txt".to_owned()),
      },
    ),
    (
      "npm install --registry https://registry.example left-pad".to_owned(),
      CommandReason::Installs {
        registry: "registry.example".to_owned(),
        package: Some("left-pad".to_owned()),
        listed_in: None,
      },
    ),
    (
      "pip install --index-url https://user:tok@pypi.example/simple requests".to_owned(),
      CommandReason::Installs {
        registry: "pypi.example".to_owned(),
        package: Some("requests".to_owned()),
        listed_in: None,
      },
    ),
    (
      "npm install --registry=registry.example left-pad".to_owned(),
      CommandReason::Installs {
        registry: "registry.example".to_owned(),
        package: Some("left-pad".to_owned()),
        listed_in: None,
      },
    ),
  ];
  for (line, reason) in cases {
    assert_eq!(folders.classify(&line).reason(), Some(reason), "{line}");
  }
}
