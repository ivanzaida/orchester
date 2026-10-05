//! The probes of the security review of `claude/agent-tools` (2026-10-05): each must ask, never run
//! by itself (`run::permitted`). Redirections into a file, `awk` programs that name other files or the
//! environment, globs and `cd`s that lead through a link out of the worktree, `cd -` and zsh's
//! `cd old new`, Windows shares, and the forms of `grep`, `jq`, `git remote` and `cargo` that reach
//! further than their words. The worktree holds `l` and `key`, links to the home folder's `.ssh`.
//! `cargo test -p orchester-core --test command_probes command_probes`.

use std::path::Path;

use orchester_core::{
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{ApprovalSubject, permitted},
};
use tempfile::TempDir;

struct Fixture {
  _root: TempDir,
  home: String,
  worktree: String,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = root.path().join("repo").join("w1");
    for folder in [
      &worktree.join("src"),
      &home.join(".ssh"),
      &home.join(".cargo"),
      &root.path().join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src").join("lib.rs"), "fn x() {}").expect("source");
    std::fs::write(home.join(".ssh").join("id_rsa"), "key").expect("key");
    std::fs::write(home.join(".cargo").join("credentials.toml"), "token").expect("token");
    link_folder(&worktree.join("l"), &home.join(".ssh"));
    link_folder(&worktree.join("key"), &home.join(".ssh"));
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&root.path().join("temp")));
    Self {
      home: slashed(&home),
      worktree: slashed(&worktree),
      scope,
      _root: root,
    }
  }

  fn command(&self, line: &str) -> ClassifiedCommand {
    let line = line.replace("{HOME}", &self.home).replace("{WT}", &self.worktree);
    ClassifiedCommand::classify(&line, &self.scope)
  }

  /// Whether `line` runs unasked in a step with `repository` access.
  fn runs_in(&self, line: &str, repository: RepositoryAccess) -> bool {
    let commands = ["cargo test", "cargo check"]
      .iter()
      .map(|entry| CommandPrefix::new(entry).expect("prefix"))
      .collect();
    let list = CommandPermission::Allowed(AllowedCommands::new(commands).expect("list"));
    let permissions = AgentPermissions::new(repository, list, NetworkAccess::Off);
    permitted(&ApprovalSubject::Command(self.command(line)), &permissions).is_some()
  }

  fn runs(&self, line: &str) -> bool {
    self.runs_in(line, RepositoryAccess::ReadOnly)
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

const ASKS: &[&str] = &[
  // Redirections that write or read a file, in every form.
  "echo hi >&out.txt",
  "cat src/lib.rs >& src/lib.rs",
  "git status; ls >&src/lib.rs",
  "cat src/lib.rs 2>&out",
  "cat src/lib.rs >&2out",
  "cat src/lib.rs &> out.txt",
  "cat src/lib.rs >| out.txt",
  "cat src/lib.rs <&{HOME}/.ssh/id_rsa",
  "cat <<< x",
  "cat <(ls)",
  "tee >(cat) < src/lib.rs",
  // awk reading other files, the environment, or starting programs.
  "awk 'BEGIN{ARGV[1]=\"{HOME}/.ssh/id_rsa\";ARGC=2}{print}'",
  "git status && awk 'BEGIN{ARGV[1]=\"{HOME}/.cargo/credentials.toml\";ARGC=2}{print}' src/lib.rs",
  "awk '{ARGV[ARGC++]=\"x\"}' src/lib.rs",
  "awk 'BEGIN{FILENAME=\"x\"}' src/lib.rs",
  "awk 'BEGIN{SYMTAB[\"ARGC\"]=2}' src/lib.rs",
  "awk 'BEGIN{print ENVIRON[\"HOME\"]}'",
  "awk '{getline line < \"x\"; print line}' src/lib.rs",
  "awk 'BEGIN { while ((\"ls\" | getline l) > 0) print l }'",
  "awk 'BEGIN{system(\"ls\")}'",
  "awk '{print > \"out.txt\"}' src/lib.rs",
  "awk -v ARGC=2 '{print}' src/lib.rs",
  "awk '{print}' ARGC=3 src/lib.rs",
  "awk '@include \"x\"; {print}' src/lib.rs",
  // A `cd` through a glob, a link, the previous folder, options and zsh's two operands.
  "cd l* && rg ''",
  "cd l* && grep -r ''",
  "cd l* && ls",
  "cd l*; find",
  "cd l && ls",
  "cd - && rg ''",
  "cd - && grep -r ''",
  "cd - && find",
  "cd -- - && ls",
  "f=-; cd $f && find",
  "cd -P src && ls",
  "cd -L l/.. && ls",
  "cd ~ && ls",
  "cd {WT} .ssh && cat id_rsa",
  "cd src lib && ls",
  // Globs that match a link out of the worktree.
  "cat l*/id_rsa",
  "cat ke?/id_rsa",
  "git status; cat ke*/id_rsa",
  "f=*; cat $f",
  "ls [kl]*",
  "cat .*/x",
  // Assignments to variables a shell treats specially or ties to another (zsh's `path` is `PATH`).
  "path=./bin; ls",
  "cdpath=/; cd x && ls",
  "fpath=x; ls",
  "IFS=x; ls",
  "BASH_ENV=x; ls",
  "PS4=x; ls",
  "ZDOTDIR=x; ls",
  "file=src; ls $file",
  // zsh's `**` descends through links at any depth.
  "cat **/id_rsa",
  "cat src/**/*.rs",
  "ls l/**",
  // Reaching further than the words say.
  "grep -R password .",
  "grep --dereference-recursive password .",
  "jq -n 'include \"x\"; 1'",
  "jq -L {HOME} -n 1",
  "git remote show origin",
  "cargo test; cargo test --config build.rustc-wrapper=sh",
  "cargo test -Zunstable-options",
  "cargo test < {HOME}/notes.txt",
  "ls\r src",
];

/// Probes of [`ASKS`] that only write files in the worktree: a step that edits files runs them, as its
/// edit tools would write there.
const WORKTREE_WRITES: &[&str] = &[
  "echo hi >&out.txt",
  "cat src/lib.rs >& src/lib.rs",
  "git status; ls >&src/lib.rs",
  "cat src/lib.rs 2>&out",
  "cat src/lib.rs >&2out",
  "cat src/lib.rs &> out.txt",
];

#[test]
fn command_probes_ask() {
  let fixture = Fixture::new();
  for line in ASKS {
    assert!(!fixture.runs(line), "`{line}` must ask");
    let edits = fixture.runs_in(line, RepositoryAccess::Write);
    assert_eq!(
      edits,
      WORKTREE_WRITES.contains(line),
      "`{line}` in a step that edits files"
    );
  }
}

#[test]
fn command_probes_keep_the_plain_forms_running() {
  let fixture = Fixture::new();
  for line in [
    "cat src/lib.rs 2>&1 | head -5",
    "cat src/lib.rs 2>/dev/null >&2",
    "cd src && ls",
    "cd src; cat lib.rs",
    "awk -F: '{print $1}' src/lib.rs",
    "awk 'NR>=1 && NR<=3' src/lib.rs",
    "awk '/ERROR/ {n++} END {print n}' src/lib.rs",
    "awk '{c[$1]++} END {for (k in c) print k, c[k]}' src/lib.rs",
    "ls s*",
  ] {
    assert!(fixture.runs(line), "`{line}` runs");
  }
}

#[test]
fn command_probes_place_links_and_shares_as_credentials() {
  let fixture = Fixture::new();
  for line in [
    "cat ke?/id_rsa",
    "cd l* && ls",
    "f=*; cat $f",
    "cd l && cat id_rsa",
    "cat l/id_rsa",
  ] {
    assert!(fixture.command(line).reads_secrets(), "`{line}` reads a credential");
  }
  if cfg!(windows) {
    for line in [
      "cat '//localhost/c$/Users/someone/.ssh/id_rsa'",
      "cat '\\\\localhost\\c$\\Users\\someone\\.ssh\\id_rsa'",
      "cat '\\\\?\\C:\\Users\\someone\\.ssh\\id_rsa'",
    ] {
      assert!(fixture.command(line).reads_secrets(), "`{line}` may read a credential");
    }
  }
  let unknown = ["cd - && ls", "cd -- - && ls", "cd {WT} .ssh && cat id_rsa"];
  for line in unknown {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "`{line}`");
  }
  assert_eq!(
    fixture.command("git remote show origin").category(),
    CommandCategory::Network
  );
  assert_eq!(
    fixture.command("git remote show -n origin").category(),
    CommandCategory::ReadOnly
  );
}
