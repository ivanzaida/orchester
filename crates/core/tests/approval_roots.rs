//! Autopilot's boundary (ORC-345, `docs/design/approval-modes.md`): a tool request reduced to its paths and git
//! effects and compared with the run's roots (worktree, Sources, the agent's own temp folders).
//! `cargo test -p orchester-core --test approval_roots approval_roots`.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  permission::{
    category::{ClassifiedCommand, CommandScope},
    roots::{AgentHomes, RootKind, RootPath, RootsCheck, RunRoots, ToolRequest, agent_temp_folders},
  },
  source::{RunSources, SourceOrigin, SourcePath},
};
use tempfile::TempDir;

const BRANCH: &str = "orchester/orc-1-run-1";

/// A main checkout `repo` with the run's worktree in `repo/.orchester/worktrees/w1`, a folder `elsewhere`, Sources
/// `docs` (with `docs/nested` listed too) and `shared`, and the agent's temp and configuration folders.
struct Fixture {
  _root: TempDir,
  root: PathBuf,
  main: PathBuf,
  worktree: PathBuf,
  elsewhere: PathBuf,
  temp: PathBuf,
  config: PathBuf,
  roots: RunRoots,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("canonical");
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
    let main = root.join("repo");
    let worktree = main.join(".orchester").join("worktrees").join("w1");
    let elsewhere = root.join("elsewhere");
    let (temp, config) = (root.join("temp"), root.join("config"));
    for folder in [
      &worktree.join("src"),
      &elsewhere,
      &root.join("docs").join("nested"),
      &root.join("shared"),
      &root.join("other"),
      &temp,
      &config,
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src").join("lib.rs"), "").expect("file");
    std::fs::write(main.join("README.md"), "").expect("file");
    link_folder(&worktree.join("out"), &elsewhere);
    let sources = sources(&[
      &root.join("docs"),
      &root.join("docs").join("nested"),
      &root.join("shared"),
    ]);
    let roots = RunRoots::new(Platform::current(), &worktree, BRANCH)
      .expect("worktree")
      .with_main_checkout(&main)
      .expect("main checkout")
      .with_sources(&sources)
      .with_agent_temp(
        AgentProvider::ClaudeCodeCli,
        AgentHomes {
          temp: &temp,
          config: &config,
        },
      );
    let scope = CommandScope::new(&worktree, &[], Some(&root.join("home")), Some(&temp));
    Self {
      _root: dir,
      root,
      main,
      worktree,
      elsewhere,
      temp,
      config,
      roots,
      scope,
    }
  }

  fn command(&self, line: &str) -> RootsCheck {
    let line = line
      .replace("{ROOT}", &slashed(&self.root))
      .replace("{ELSE}", &slashed(&self.elsewhere))
      .replace("{MAIN}", &slashed(&self.main));
    let command = ClassifiedCommand::classify(&line, &self.scope);
    self.roots.check(&ToolRequest::Command(&command))
  }

  fn file(&self, path: &str) -> RootsCheck {
    self.roots.check(&ToolRequest::Files(&[path.to_owned()]))
  }

  fn outside(&self, path: &Path) -> RootsCheck {
    RootsCheck::Outside {
      path: path.to_path_buf(),
    }
  }
}

fn sources(paths: &[&Path]) -> RunSources {
  let paths: Vec<SourcePath> = paths
    .iter()
    .map(|path| SourcePath::new(&path.to_string_lossy()).expect("source"))
    .collect();
  RunSources::union(paths.iter().map(|path| (path, SourceOrigin::Project)))
}

/// A link to the folder `target` at `link`: a symbolic link on Unix, a junction on Windows.
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

#[test]
fn approval_roots_inside_the_worktree() {
  let fixture = Fixture::new();
  for path in ["src/lib.rs", "src/new.rs", "./src/../src/lib.rs"] {
    assert_eq!(fixture.file(path), RootsCheck::Inside, "{path}");
  }
  let absolute = fixture.worktree.join("src").join("lib.rs");
  assert_eq!(fixture.file(&absolute.to_string_lossy()), RootsCheck::Inside);
  for line in [
    "cat src/lib.rs > src/copy.rs",
    "cd src && ls",
    "git status",
    "git commit -m 'fix /api route'",
    "git checkout src/lib.rs",
    "git fetch origin",
    "git reset --hard HEAD~1",
    "cargo test -p x --test y z",
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Inside, "{line}");
  }
  // An MCP call of a server that is no local filesystem names no path.
  assert_eq!(
    fixture.roots.check(&ToolRequest::McpTool { paths: &[] }),
    RootsCheck::Inside
  );
}

#[test]
fn approval_roots_outside_the_roots() {
  let fixture = Fixture::new();
  let target = fixture.elsewhere.join("notes.md");
  assert_eq!(fixture.file(&target.to_string_lossy()), fixture.outside(&target));
  assert_eq!(fixture.command("cat {ELSE}/notes.md"), fixture.outside(&target));
  assert_eq!(fixture.command("echo x > {ELSE}/notes.md"), fixture.outside(&target));
  assert_eq!(fixture.command("cd {ELSE} && ls"), fixture.outside(&fixture.elsewhere));
  // Through a link (a junction on Windows) in the worktree.
  assert_eq!(fixture.file("out/notes.md"), fixture.outside(&target));
  let mcp = [target.to_string_lossy().into_owned()];
  assert_eq!(
    fixture.roots.check(&ToolRequest::McpTool { paths: &mcp }),
    fixture.outside(&target)
  );
  // The first reason found: the outside path comes before the push of the same part.
  assert_eq!(
    fixture.command("git -C {ELSE} push"),
    fixture.outside(&fixture.elsewhere)
  );
}

#[test]
fn approval_roots_main_checkout() {
  let fixture = Fixture::new();
  let readme = fixture.main.join("README.md");
  assert_eq!(
    fixture.file(&readme.to_string_lossy()),
    RootsCheck::MainCheckout { path: readme.clone() }
  );
  assert_eq!(
    fixture.file("../../../README.md"),
    RootsCheck::MainCheckout { path: readme }
  );
  let shared = RootsCheck::MainCheckout {
    path: fixture.main.join(".git"),
  };
  for line in [
    "git config core.autocrlf false",
    "git config --unset core.bare",
    "git config --unset-all remote.origin.fetch",
    "git config --remove-section remote.origin",
    "git config --rename-section remote.origin remote.old",
    "git config --add remote.origin.fetch x",
    "git config --replace-all core.x y",
    "git config --edit",
    "git config -e",
    "git config unset core.bare",
    "git config edit",
    "git remote add fork x",
    "git gc",
    "git worktree prune",
  ] {
    assert_eq!(fixture.command(line), shared, "{line}");
  }
  for line in [
    "git config --get core.autocrlf",
    "git config core.autocrlf",
    "git config list",
    "git config get core.autocrlf",
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Inside, "{line}");
  }
}

#[test]
fn approval_roots_push() {
  let fixture = Fixture::new();
  for line in [
    "git push",
    "git push origin orchester/orc-1-run-1",
    "git push --force origin HEAD",
    "git status && git push -u origin HEAD",
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Push, "{line}");
  }
  assert_eq!(
    fixture.command("git stash push -m wip"),
    RootsCheck::OtherRef {
      name: "refs/stash".to_owned()
    }
  );
}

#[test]
fn approval_roots_other_refs() {
  let fixture = Fixture::new();
  let other = |name: &str| RootsCheck::OtherRef { name: name.to_owned() };
  for (line, check) in [
    ("git branch -D other", other("other")),
    ("git branch -d refs/heads/other", other("other")),
    ("git branch new-one", other("new-one")),
    ("git branch -m renamed", other("renamed")),
    ("git tag v1.0", other("refs/tags/v1.0")),
    ("git tag -d v1.0", other("refs/tags/v1.0")),
    ("git checkout -b side", other("side")),
    ("git checkout master", other("master")),
    ("git switch master", other("master")),
    ("git switch -c side", other("side")),
    ("git stash", other("refs/stash")),
    ("git notes add -m x", other("refs/notes/commits")),
    ("git update-ref refs/heads/other HEAD", other("other")),
    ("git fetch origin master:master", other("master")),
    ("git rebase master other", other("other")),
    ("git bisect start", other("refs/bisect")),
  ] {
    assert_eq!(fixture.command(line), check, "{line}");
  }
  for line in [
    "git branch",
    "git branch --list 'orc-*'",
    "git branch -D orchester/orc-1-run-1",
    "git tag --list",
    "git stash list",
    "git switch orchester/orc-1-run-1",
    "git checkout --detach",
    "git rebase origin/master",
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Inside, "{line}");
  }
}

#[test]
fn approval_roots_unknown() {
  let fixture = Fixture::new();
  for line in [
    "",
    "cat ~bob/notes",
    "eval \"$CMD\"",
    "git co other",
    "git -c alias.st=push st",
    "git --namespace=x branch y",
    "git update-ref --stdin",
    "GIT_DIR=x git status",
    "git switch -",
    // A variable of git's own set before it: which repository git works on cannot be told.
    "export GIT_DIR={ROOT}/other/.git && git commit -am x",
    "export GIT_WORK_TREE={ROOT}/other; git checkout -- .",
    "GIT_DIR=src/.git; export GIT_DIR; git reset --hard",
    "declare -x GIT_DIR=/x/.git && git status",
  ] {
    assert_eq!(fixture.command(line), RootsCheck::Unknown, "{line:?}");
  }
  // A variable of another name changes nothing of git's.
  assert_eq!(
    fixture.command("export RUST_LOG=debug && git status"),
    RootsCheck::Inside
  );
  for path in ["", "~/notes.md", "~bob/x"] {
    assert_eq!(fixture.file(path), RootsCheck::Unknown, "{path:?}");
  }
  // Without the main checkout, a write of the repository's shared state cannot be told.
  let roots = RunRoots::new(Platform::current(), &fixture.worktree, BRANCH).expect("worktree");
  let command = ClassifiedCommand::classify("git gc", &fixture.scope);
  assert_eq!(roots.check(&ToolRequest::Command(&command)), RootsCheck::Unknown);
}

#[test]
fn approval_roots_git_c_pointing_out() {
  let fixture = Fixture::new();
  let other = fixture.root.join("other");
  assert_eq!(fixture.command("git -C {ROOT}/other status"), fixture.outside(&other));
  assert_eq!(
    fixture.command("git -C {ROOT}/other branch -D x"),
    fixture.outside(&other)
  );
  assert_eq!(
    fixture.command("git --git-dir={ROOT}/other/.git log"),
    fixture.outside(&other.join(".git"))
  );
  assert_eq!(
    fixture.command("git --work-tree {ROOT}/other status"),
    fixture.outside(&other)
  );
  assert_eq!(fixture.command("git -C src status"), RootsCheck::Inside);
}

#[test]
fn approval_roots_sources_nested_and_overlapping() {
  let fixture = Fixture::new();
  let docs = fixture.root.join("docs");
  for path in [
    docs.join("a.md"),
    docs.join("nested").join("b.md"),
    fixture.root.join("shared").join("c.md"),
  ] {
    assert_eq!(fixture.file(&path.to_string_lossy()), RootsCheck::Inside, "{path:?}");
  }
  // A folder beside a source, whose name starts like it, is not in it.
  let beside = fixture.root.join("docs-old").join("a.md");
  assert_eq!(fixture.file(&beside.to_string_lossy()), fixture.outside(&beside));
  assert_eq!(
    fixture.command("rg todo {ROOT}/docs {ROOT}/docs/nested"),
    RootsCheck::Inside
  );
  // The same source listed twice, once through a link to it, overlaps itself.
  let link = fixture.root.join("docs-link");
  link_folder(&link, &docs);
  let roots = RunRoots::new(Platform::current(), &fixture.worktree, BRANCH)
    .expect("worktree")
    .with_sources(&sources(&[&docs, &link, &docs.join("nested")]));
  let kinds: Vec<RootKind> = roots.roots().iter().map(|root| root.kind()).collect();
  assert_eq!(
    kinds,
    [RootKind::Worktree, RootKind::Source, RootKind::Source, RootKind::Source]
  );
  assert_eq!(roots.roots()[1].path(), roots.roots()[2].path());
  let through_link = [link.join("nested").join("b.md").to_string_lossy().into_owned()];
  assert_eq!(roots.check(&ToolRequest::Files(&through_link)), RootsCheck::Inside);
}

#[test]
fn approval_roots_agent_temp_folder() {
  let fixture = Fixture::new();
  let folder: String = fixture
    .worktree
    .to_string_lossy()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
    .collect();
  let task = fixture
    .temp
    .join("claude")
    .join(&folder)
    .join("tasks")
    .join("b1.output");
  let session = fixture.config.join("projects").join(&folder).join("s1.jsonl");
  for path in [&task, &session] {
    assert_eq!(fixture.file(&path.to_string_lossy()), RootsCheck::Inside, "{path:?}");
  }
  assert_eq!(
    fixture.command(&format!("tail -n 20 {}", slashed(&task))),
    RootsCheck::Inside
  );
  // The rest of the temporary and configuration folders, the CLI's sign-in among them, is not the run's.
  let credentials = fixture.config.join(".credentials.json");
  assert_eq!(
    fixture.file(&credentials.to_string_lossy()),
    fixture.outside(&credentials)
  );
  let other_run = fixture
    .temp
    .join("claude")
    .join("C--other")
    .join("tasks")
    .join("x.output");
  assert_eq!(fixture.file(&other_run.to_string_lossy()), fixture.outside(&other_run));

  let homes = AgentHomes {
    temp: &fixture.temp,
    config: &fixture.config,
  };
  assert_eq!(
    agent_temp_folders(AgentProvider::CodexCli, Platform::current(), &fixture.worktree, homes),
    [fixture.config.join("sessions")]
  );
  assert!(
    agent_temp_folders(
      AgentProvider::AnthropicApi,
      Platform::current(),
      &fixture.worktree,
      homes
    )
    .is_empty()
  );
  let kinds: Vec<RootKind> = fixture.roots.roots().iter().map(|root| root.kind()).collect();
  assert_eq!(&kinds[kinds.len() - 2..], [RootKind::AgentTemp, RootKind::AgentTemp]);
}

/// Windows paths, read by Windows' rules on either platform.
#[test]
fn approval_roots_windows_normalization() {
  let roots = RunRoots::new(Platform::Windows, Path::new(r"C:\Repo\.orchester\worktrees\w1"), BRANCH)
    .expect("worktree")
    .with_main_checkout(Path::new("C:/Repo"))
    .expect("main checkout");
  let check = |path: &str| roots.check(&ToolRequest::Files(&[path.to_owned()]));
  for path in [
    r"C:\Repo\.orchester\worktrees\w1\src\lib.rs",
    r"c:\repo\.ORCHESTER\Worktrees\W1\src\lib.rs",
    r"\\?\C:\Repo\.orchester\worktrees\w1\src\lib.rs",
    "C:/Repo/.orchester/worktrees/w1/src/lib.rs",
    r"C:\Repo\.orchester\worktrees\w1\src\..\README.md",
    r"C:\Repo\.orchester\worktrees\w1\notes.txt.",
    r"C:\Repo\.orchester\worktrees\w1\notes.txt::$DATA",
    r"src\lib.rs",
  ] {
    assert_eq!(check(path), RootsCheck::Inside, "{path}");
  }
  assert_eq!(
    check(r"C:\Repo\.orchester\worktrees\w1\..\w2\x"),
    RootsCheck::MainCheckout {
      path: PathBuf::from(r"C:\Repo\.orchester\worktrees\w2\x")
    }
  );
  assert!(matches!(check(r"..\..\..\..\Users\x"), RootsCheck::Outside { .. }));
  assert!(matches!(check(r"C:\Repo-other\x"), RootsCheck::Outside { .. }));
  // Rooted without a drive, or a drive without a root: which file it is cannot be told. A file tool reads Git Bash's
  // `/c/x` on the current drive, as `\c\x`.
  for path in [
    r"\Windows\x",
    "/etc/hosts",
    "C:x",
    "/c/Repo/.orchester/worktrees/w1/src/lib.rs",
  ] {
    assert_eq!(check(path), RootsCheck::Unknown, "{path}");
  }
}

/// macOS paths, read by macOS' rules on either platform.
#[test]
fn approval_roots_macos_normalization() {
  let temp = Path::new("/private/var/folders/ab/T");
  let roots = RunRoots::new(
    Platform::MacOs,
    Path::new("/Users/ana/kontur/.orchester/worktrees/w1"),
    BRANCH,
  )
  .expect("worktree")
  .with_agent_temp(
    AgentProvider::ClaudeCodeCli,
    AgentHomes {
      temp,
      config: Path::new("/Users/ana/.claude"),
    },
  );
  let check = |path: &str| roots.check(&ToolRequest::Files(&[path.to_owned()]));
  let folder = "-Users-ana-kontur--orchester-worktrees-w1";
  for path in [
    "/Users/ana/kontur/.orchester/worktrees/w1/src/main.rs",
    "/Users/Ana/Kontur/.orchester/worktrees/W1/src/main.rs",
    "src/../Cargo.toml",
    &format!("/var/folders/ab/T/claude/{folder}/tasks/b1.output"),
    &format!("/private/var/folders/ab/T/claude/{folder}/tasks/b1.output"),
    &format!("/Users/ana/.claude/projects/{folder}/s1.jsonl"),
  ] {
    assert_eq!(check(path), RootsCheck::Inside, "{path}");
  }
  assert_eq!(
    check("/Users/ana/kontur/.orchester/worktrees/w1/../w2/x"),
    RootsCheck::Outside {
      path: PathBuf::from("/Users/ana/kontur/.orchester/worktrees/w2/x")
    }
  );
  let alias = |path: &str| RootPath::new(Platform::MacOs, path, None).expect("absolute");
  assert_eq!(alias("/private/tmp/x"), alias("/tmp/x"));
  assert_eq!(alias("/private/tmp/x").path(), Path::new("/tmp/x"));
  assert!(RootPath::new(Platform::MacOs, "relative", None).is_none());
}

/// `..` after a link: on POSIX the parent of the link's target, on Windows the folder that holds the link, as Win32
/// takes `..` away from the text first.
#[test]
fn approval_roots_link_then_parent() {
  let fixture = Fixture::new();
  let through_link = fixture.file("out/../x");
  #[cfg(unix)]
  assert_eq!(through_link, fixture.outside(&fixture.root.join("x")));
  #[cfg(windows)]
  assert_eq!(through_link, RootsCheck::Inside);
  #[cfg(unix)]
  assert_eq!(
    fixture.file("out/../elsewhere/notes.md"),
    fixture.outside(&fixture.elsewhere.join("notes.md"))
  );
}
