//! The Orchestrator chat's git tools (`docs/design/orchestrator-toolset.md`, decision 9): typed tools rather than a
//! free-form `git <args>`, which could not be told safe. Reads are always open; writes that lose nothing run only
//! while the person's switch *Let the Orchestrator write with git* is on (`write_off` otherwise); what can lose work is
//! only ever proposed, as a card only a person applies. They work in the project's checkout, or in a run's worktree
//! named by its id or number, which is written to only once the run ended (`run_active`). git never waits for a
//! person here: no prompt for credentials (`auth_failed` at once), no editor, and network work is stopped after
//! [`NETWORK_TIMEOUT`] (`timed_out`).

mod args;
pub(super) mod chat;
mod command;
mod describe;
mod names;
mod place;
mod propose;
mod read;
mod write;

use std::{path::Path, sync::Arc, time::Duration};

use orchester_core::orchestrator::git::{GitChange, GitOperation, GitWrite};
use orchester_exec::programs::Programs;
use rmcp::{
  handler::server::common::schema_for_type,
  model::{JsonObject, Tool, ToolAnnotations},
  schemars::JsonSchema,
};
use serde_json::Value;

use self::{
  args::{Nothing, parse},
  command::{Git, refused},
  place::{Place, Use},
  write::Ran,
};
use crate::{app_mcp::ToolError, messages::Message};

/// How long a pull, push, fetch or a proposal that reaches a remote may take before git is stopped.
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

/// The tool that proposes what can lose work.
pub const GIT_PROPOSE: &str = "git_propose";

/// The reads, always open.
const READS: [&str; 7] = [
  "git_status",
  "git_log",
  "git_diff",
  "git_show",
  "git_branches",
  "git_blame",
  "git_remotes",
];

/// The writes, behind the person's switch.
const WRITES: [(&str, GitWrite); 9] = [
  ("git_commit", GitWrite::Commit),
  ("git_create_branch", GitWrite::CreateBranch),
  ("git_switch", GitWrite::Switch),
  ("git_pull", GitWrite::Pull),
  ("git_push", GitWrite::Push),
  ("git_merge", GitWrite::Merge),
  ("git_revert", GitWrite::Revert),
  ("git_tag", GitWrite::Tag),
  ("git_fetch", GitWrite::Fetch),
];

/// Every git tool's name.
pub const NAMES: [&str; 17] = [
  "git_status",
  "git_log",
  "git_diff",
  "git_show",
  "git_branches",
  "git_blame",
  "git_remotes",
  "git_commit",
  "git_create_branch",
  "git_switch",
  "git_pull",
  "git_push",
  "git_merge",
  "git_revert",
  "git_tag",
  "git_fetch",
  GIT_PROPOSE,
];

/// What a git tool does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
  Read,
  Write(GitWrite),
  Propose,
}

/// What git tool `name` does; `None` for a tool that is not one.
pub fn kind(name: &str) -> Option<Kind> {
  if READS.contains(&name) {
    return Some(Kind::Read);
  }
  if name == GIT_PROPOSE {
    return Some(Kind::Propose);
  }
  WRITES
    .iter()
    .find(|(tool, _)| *tool == name)
    .map(|(_, write)| Kind::Write(*write))
}

/// The git tools as the chat's `tools/list` answers.
pub fn tools() -> Vec<Tool> {
  let read = |name: &'static str, description: &'static str, schema| {
    Tool::new(name, description, schema).with_annotations(ToolAnnotations::new().read_only(true).open_world(false))
  };
  let write = |name: &'static str, description: String, schema, network: bool| {
    Tool::new(name, description, schema).with_annotations(
      ToolAnnotations::new()
        .read_only(false)
        .destructive(false)
        .open_world(network),
    )
  };
  vec![
    read("git_status", describe::STATUS, schema::<Nothing>()),
    read("git_log", describe::LOG, schema::<args::Log>()),
    read("git_diff", describe::DIFF, schema::<args::Diff>()),
    read("git_show", describe::SHOW, schema::<args::Show>()),
    read("git_branches", describe::BRANCHES, schema::<Nothing>()),
    read("git_blame", describe::BLAME, schema::<args::Blame>()),
    read("git_remotes", describe::REMOTES, schema::<Nothing>()),
    write("git_commit", describe::commit(), schema::<args::Commit>(), false),
    write(
      "git_create_branch",
      describe::create_branch(),
      schema::<args::CreateBranch>(),
      false,
    ),
    write("git_switch", describe::switch(), schema::<args::Switch>(), false),
    write("git_pull", describe::pull(), schema::<args::Pull>(), true),
    write("git_push", describe::push(), schema::<args::Push>(), true),
    write("git_merge", describe::merge(), schema::<args::Merge>(), false),
    write("git_revert", describe::revert(), schema::<args::Revert>(), false),
    write("git_tag", describe::tag(), schema::<args::Tag>(), false),
    write("git_fetch", describe::fetch(), schema::<args::Fetch>(), true),
    write(
      GIT_PROPOSE,
      describe::PROPOSE.to_owned(),
      schema::<args::Propose>(),
      false,
    ),
  ]
}

/// `T`'s input schema, with the `run` every git tool takes.
fn schema<T: JsonSchema + 'static>() -> Arc<JsonObject> {
  Arc::new(args::with_run((*schema_for_type::<T>()).clone()))
}

/// What a git tool's call did.
#[derive(Debug)]
pub enum Called {
  /// A read's answer.
  Read(Value),
  /// A write that ran: its answer, or the refusal of what git did not do, and what ran.
  Wrote {
    result: Result<Value, ToolError>,
    change: GitChange,
  },
  /// A proposal of what can lose work, for the person.
  Proposed(GitChange),
}

/// Calls git tool `name` with `arguments` (their `project` the chat's own, already checked) in the project at `root`,
/// with `programs`; `writes` is the person's switch. Refused before git runs for a write while the switch is off, a
/// write in a run's worktree before the run ended, a folder that is not a repository, and arguments git would not
/// take. Blocks on git and SQLite.
pub fn call(
  programs: &Programs,
  root: &Path,
  (name, arguments): (&str, &JsonObject),
  writes: bool,
) -> Result<Called, ToolError> {
  let kind = kind(name).ok_or_else(|| ToolError::InvalidArguments(format!("no git tool is named {name}")))?;
  if matches!(kind, Kind::Write(_)) && !writes {
    return Err(refused("write_off", Message::new("orchestrator.git.writeOff")));
  }
  let mut arguments = arguments.clone();
  arguments.remove("project");
  let run = match arguments.remove("run") {
    None | Some(Value::Null) => None,
    Some(Value::String(run)) => Some(run),
    Some(other) => {
      return Err(ToolError::InvalidArguments(format!(
        "run: {other} is not a run's id or number"
      )));
    }
  };
  let using = if kind == Kind::Read { Use::Read } else { Use::Write };
  let place = place::resolve(root, run.as_deref(), using)?;
  let quick = Git {
    runner: programs.runner(),
    dir: &place.dir,
  };
  repository(&quick)?;
  match kind {
    Kind::Read => read_tool(&quick, name, &arguments).map(Called::Read),
    Kind::Propose => {
      let asked: args::Propose = parse(&arguments)?;
      propose::propose(&quick, asked.operation, place.at).map(Called::Proposed)
    }
    Kind::Write(write) => Ok(write_tool(programs, &place, write, &arguments)),
  }
}

/// Applies the proposal `change`, a person's decision, in the project at `root`: its place is checked again (a run
/// that went on since is refused), then what it stands on. Blocks on git and SQLite.
pub fn apply(programs: &Programs, root: &Path, change: &GitChange) -> Result<Value, ToolError> {
  let place = place::again(root, change.place)?;
  let network = programs.timed(NETWORK_TIMEOUT);
  let (local, network) = (
    Git {
      runner: programs.git_work(),
      dir: &place.dir,
    },
    Git {
      runner: &*network,
      dir: &place.dir,
    },
  );
  repository(&local)?;
  let mut ran = Ran {
    local: &local,
    network: &network,
    commands: Vec::new(),
    said: None,
  };
  propose::apply(&mut ran, change)
}

/// Refused unless git finds a repository in `git`'s folder.
fn repository(git: &Git) -> Result<(), ToolError> {
  let output = git.run(&["rev-parse", "--git-dir"], None)?;
  if output.success() {
    return Ok(());
  }
  Err(refused(
    "not_a_repository",
    Message::new("orchestrator.git.notRepository"),
  ))
}

fn read_tool(git: &Git, name: &str, arguments: &JsonObject) -> Result<Value, ToolError> {
  match name {
    "git_status" => read::status(git, &parse(arguments)?),
    "git_log" => read::log(git, &parse(arguments)?),
    "git_diff" => read::diff(git, &parse(arguments)?),
    "git_show" => read::show(git, &parse(arguments)?),
    "git_branches" => read::branches(git, &parse(arguments)?),
    "git_blame" => read::blame(git, &parse(arguments)?),
    _ => read::remotes(git, &parse(arguments)?),
  }
}

/// Runs the write `write` at `place`: what it answered or why git refused, and every command it ran.
fn write_tool(programs: &Programs, place: &Place, write: GitWrite, arguments: &JsonObject) -> Called {
  let network = programs.timed(NETWORK_TIMEOUT);
  let local = Git {
    runner: programs.git_work(),
    dir: &place.dir,
  };
  let network = Git {
    runner: &*network,
    dir: &place.dir,
  };
  let mut ran = Ran {
    local: &local,
    network: &network,
    commands: Vec::new(),
    said: None,
  };
  let result = match write {
    GitWrite::Commit => parse(arguments).and_then(|asked| write::commit(&mut ran, &asked)),
    GitWrite::CreateBranch => parse(arguments).and_then(|asked| write::create_branch(&mut ran, &asked)),
    GitWrite::Switch => parse(arguments).and_then(|asked| write::switch(&mut ran, &asked)),
    GitWrite::Pull => parse(arguments).and_then(|asked| write::pull(&mut ran, &asked)),
    GitWrite::Push => parse(arguments).and_then(|asked| write::push(&mut ran, &asked)),
    GitWrite::Merge => parse(arguments).and_then(|asked| write::merge(&mut ran, &asked)),
    GitWrite::Revert => parse(arguments).and_then(|asked| write::revert(&mut ran, &asked)),
    GitWrite::Tag => parse(arguments).and_then(|asked| write::tag(&mut ran, &asked)),
    GitWrite::Fetch => parse(arguments).and_then(|asked| write::fetch(&mut ran, &asked)),
  };
  let change = GitChange {
    place: place.at,
    operation: GitOperation::Write(write),
    commands: ran.commands,
    said: ran.said,
    basis: String::new(),
    loses: Vec::new(),
  };
  Called::Wrote { result, change }
}
