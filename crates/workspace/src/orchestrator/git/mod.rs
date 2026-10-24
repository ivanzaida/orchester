//! The Orchestrator chat's git tools (`docs/design/orchestrator-toolset.md`, decision 9): typed tools rather than a
//! free-form `git <args>`, which could not be told safe. Reads are always open; writes that lose nothing run only
//! while the person's switch *Let the Orchestrator write with git* is on (`write_off` otherwise); what can lose work is
//! only ever proposed, as a card only a person applies. They work in the project's checkout, or in a run's worktree
//! named by its id or number, which is written to only once the run ended (`run_active`). git never waits for a
//! person here: no prompt for credentials (`auth_failed` at once), ssh in batch mode, no editor; network work is
//! stopped after [`Timeouts::STANDARD`]'s network time and local writes after its local time (`timed_out`). A write
//! holds Orchester's claim on the checkout while it runs and is refused while local integration or a resolution holds
//! it (`checkout_reserved`, [`place::claim`]).

mod args;
pub(super) mod chat;
mod command;
mod describe;
mod names;
mod place;
mod propose;
mod read;
mod ssh;
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

pub use self::ssh::{batch_ssh_command, batch_ssh_program};
use self::{
  args::{Nothing, parse},
  command::{Git, refused},
  place::{Place, Use},
  write::Ran,
};
use crate::{app_mcp::ToolError, messages::Message};

/// How long git may take before it is stopped (`timed_out`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeouts {
  /// A local write, its hooks included.
  pub local: Duration,
  /// A pull, push, fetch or a proposal that reaches a remote.
  pub network: Duration,
}

impl Timeouts {
  /// The chat's: long enough for a commit's hooks and a slow remote, short enough that a git waiting on something
  /// never holds the checkout's claim for long.
  pub const STANDARD: Self = Self {
    local: Duration::from_secs(5 * 60),
    network: Duration::from_secs(120),
  };
}

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
  (writes, timeouts): (bool, Timeouts),
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
  let quick = Git::new(programs.runner(), &place.dir);
  repository(&quick)?;
  match kind {
    Kind::Read => read_tool((programs, &quick), name, &arguments).map(Called::Read),
    Kind::Propose => {
      let asked: args::Propose = parse(&arguments)?;
      propose::propose(&quick, asked.operation, place.at).map(Called::Proposed)
    }
    Kind::Write(write) => {
      let _claim = place::claim(root, &place, changes_checkout(write))?;
      write_tool((programs, timeouts), &place, write, &arguments)
    }
  }
}

/// Applies the proposal `change`, a person's decision, in the project at `root`: its place is checked again (a run
/// that went on since is refused), it is claimed as a write ([`place::claim`]), then what it stands on is checked.
/// Blocks on git and SQLite.
pub fn apply(programs: &Programs, root: &Path, change: &GitChange, timeouts: Timeouts) -> Result<Value, ToolError> {
  let place = place::again(root, change.place)?;
  repository(&Git::new(programs.runner(), &place.dir))?;
  let lease = match &change.operation {
    GitOperation::Propose(proposal) => propose::changes_checkout(proposal),
    GitOperation::Write(write) => changes_checkout(*write),
  };
  let _claim = place::claim(root, &place, lease)?;
  let (result, _) = ran_at((programs, timeouts), &place, |ran| propose::apply(ran, change))?;
  result
}

/// Whether `write` changes the working tree or the index, so it holds the local-integration lease while it runs: all
/// but a push, a fetch and a tag, which change refs or a remote only and only check that nothing reserves the
/// checkout. A pull holds it across its fetch too, up to the network timeout (decision 9).
const fn changes_checkout(write: GitWrite) -> bool {
  !matches!(write, GitWrite::Push | GitWrite::Fetch | GitWrite::Tag)
}

/// The commands a write ran, and what git last said.
type WhatRan = (Vec<String>, Option<String>);

/// Runs `work` with the git of `place`: local work stopped after `timeouts.local`, network work after
/// `timeouts.network` with ssh in batch mode; its result and what ran.
fn ran_at<T>(
  (programs, timeouts): (&Programs, Timeouts),
  place: &Place,
  work: impl FnOnce(&mut Ran) -> T,
) -> Result<(T, WhatRan), ToolError> {
  let (local_runner, network_runner) = (programs.timed(timeouts.local), programs.timed(timeouts.network));
  let local = Git::new(&*local_runner, &place.dir);
  let mut network = Git::new(&*network_runner, &place.dir);
  network.ssh = Some(ssh::batch_ssh(&local)?);
  let mut ran = Ran {
    local: &local,
    network: &network,
    commands: Vec::new(),
    said: None,
  };
  let result = work(&mut ran);
  Ok((result, (ran.commands, ran.said)))
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

fn read_tool((programs, git): (&Programs, &Git), name: &str, arguments: &JsonObject) -> Result<Value, ToolError> {
  match name {
    "git_status" => read::status(git, &parse(arguments)?),
    "git_log" => read::log((programs, git), &parse(arguments)?),
    "git_diff" => read::diff((programs, git), &parse(arguments)?),
    "git_show" => read::show((programs, git), &parse(arguments)?),
    "git_branches" => read::branches(git, &parse(arguments)?),
    "git_blame" => read::blame((programs, git), &parse(arguments)?),
    _ => read::remotes(git, &parse(arguments)?),
  }
}

/// Runs the write `write` at `place`: what it answered or why git refused, and every command it ran.
fn write_tool(
  (programs, timeouts): (&Programs, Timeouts),
  place: &Place,
  write: GitWrite,
  arguments: &JsonObject,
) -> Result<Called, ToolError> {
  let (result, (commands, said)) = ran_at((programs, timeouts), place, |ran| match write {
    GitWrite::Commit => parse(arguments).and_then(|asked| write::commit(ran, &asked)),
    GitWrite::CreateBranch => parse(arguments).and_then(|asked| write::create_branch(ran, &asked)),
    GitWrite::Switch => parse(arguments).and_then(|asked| write::switch(ran, &asked)),
    GitWrite::Pull => parse(arguments).and_then(|asked| write::pull(ran, &asked)),
    GitWrite::Push => parse(arguments).and_then(|asked| write::push(ran, &asked)),
    GitWrite::Merge => parse(arguments).and_then(|asked| write::merge(ran, &asked)),
    GitWrite::Revert => parse(arguments).and_then(|asked| write::revert(ran, &asked)),
    GitWrite::Tag => parse(arguments).and_then(|asked| write::tag(ran, &asked)),
    GitWrite::Fetch => parse(arguments).and_then(|asked| write::fetch(ran, &asked)),
  })?;
  let change = GitChange {
    place: place.at,
    operation: GitOperation::Write(write),
    commands,
    said,
    basis: String::new(),
    loses: Vec::new(),
  };
  Ok(Called::Wrote { result, change })
}
