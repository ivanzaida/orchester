//! Orchester's typed tools (`orchester_core::orchester_tools`): strict input, the program each git call
//! runs with every value one argument of its own, the policy table, reads outside the worktree that never
//! reach a credential location or Orchester's own state, answers cut to a size, and how each CLI is told
//! about them.

mod index;
mod input;
mod outside;
mod policy;
mod programs;
mod skill_reads;
mod skills;

use std::{
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::invocation::Platform,
  orchester_tools::{
    OutsideReads, ProjectTool, StepTools, ToolCall, ToolCallError, ToolContext, ToolProgram, ToolWork,
  },
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, category::CommandScope},
  resource::{CommandAction, CommandArg, CommandEnvironment, CommandProgram, RepositoryAccess},
};
use serde_json::Value;
use tempfile::TempDir;

pub fn permissions(repository: RepositoryAccess) -> AgentPermissions {
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::default()),
    NetworkAccess::Off,
  )
}

pub fn writing() -> StepTools {
  StepTools::for_step(&permissions(RepositoryAccess::Write), &[])
}

pub fn reading() -> StepTools {
  StepTools::for_step(&permissions(RepositoryAccess::ReadOnly), &[])
}

pub fn parse(name: &str, arguments: &Value) -> Result<ToolCall, ToolCallError> {
  ToolCall::parse(name, arguments, &writing())
}

/// Folders of one test: a worktree, a home folder with credentials, a temporary folder and a dependency's
/// sources outside them all.
pub(crate) struct Folders {
  _root: TempDir,
  pub worktree: PathBuf,
  pub home: PathBuf,
  pub dependency: PathBuf,
  pub private: PathBuf,
}

impl Folders {
  pub(crate) fn new() -> Self {
    let root = tempfile::tempdir().expect("a temporary folder");
    let base = root.path().to_path_buf();
    let folder = |name: &str| {
      let path = base.join(name);
      fs::create_dir_all(&path).expect("a folder");
      path
    };
    let (worktree, home, dependency, private) = (folder("worktree"), folder("home"), folder("deps"), folder("private"));
    for (path, text) in [
      (home.join(".ssh").join("id_ed25519"), "secret key"),
      (
        home.join("AppData").join("Roaming").join("gh").join("hosts.yml"),
        "token",
      ),
      (home.join(".cargo").join("registry").join("lib.rs"), "fn cached() {}\n"),
      (
        dependency.join("src").join("lib.rs"),
        "line 1\nline 2\nline 3\nline 4\n",
      ),
      (
        dependency.join(".git").join("config"),
        "[remote]\nurl = https://user:token@host/x\n",
      ),
      (private.join("invocation.json"), "{\"token\":\"x\"}"),
    ] {
      fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
      fs::write(path, text).expect("a file");
    }
    fs::write(dependency.join("blob.bin"), b"a\0b\n").expect("a file");
    Self {
      _root: root,
      worktree,
      home,
      dependency,
      private,
    }
  }

  pub fn reads(&self) -> OutsideReads {
    let scope = CommandScope::new(&self.worktree, &[], Some(&self.home), None);
    OutsideReads::new(scope, std::slice::from_ref(&self.private))
  }
}

pub fn text(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

pub fn project_tool(name: &str, program: &str) -> ProjectTool {
  let args = ["--quiet", "a; rm -rf /"].map(|arg| CommandArg::new(arg).expect("an argument"));
  let action = CommandAction::new(CommandProgram::new(program).expect("a program"), args.to_vec());
  ProjectTool::new(name, "runs the tests", action)
}

pub fn context<'a>(worktree: &'a Path, reads: &'a OutsideReads, tools: &'a [ProjectTool]) -> ToolContext<'a> {
  static ENVIRONMENT: std::sync::LazyLock<CommandEnvironment> = std::sync::LazyLock::new(CommandEnvironment::default);
  ToolContext {
    worktree,
    inherited: &[],
    project: &ENVIRONMENT,
    outside: reads,
    project_tools: tools,
    platform: Platform::current(),
    git_dir: Path::new("/repo/.git/worktrees/run"),
    hooks: Path::new("/orchester-private/no-hooks"),
  }
}

pub fn program_of(call: &ToolCall, context: &ToolContext<'_>) -> ToolProgram {
  match call.work(context) {
    Ok(ToolWork::Run(program)) => *program,
    other => panic!("{call:?} runs a program, not {other:?}"),
  }
}
