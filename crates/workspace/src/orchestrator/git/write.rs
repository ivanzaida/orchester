//! The chat's git writes, run only while the person's switch *Let the Orchestrator write with git* is on: none of them
//! loses work. A commit, a new branch, a switch git makes only when it keeps every local change, a pull that only
//! fast-forwards unless the person asked for a merge, a push that never forces, a merge or revert that a conflict
//! stops and undoes, a tag and a fetch. Branches, tags and start points are plain names and remotes ones the
//! repository lists ([`super::names`]); nothing but a commit runs while a merge or rebase is in progress, and no commit
//! while a file is in conflict. Each command run is recorded ([`Ran`]) so its card shows what ran and what git said.

use serde_json::{Value, json};

use super::{
  args::{Commit, CreateBranch, Fetch, Merge, Pull, Push, Revert, Switch, Tag},
  command::{Git, refusal, refused, said_of_success, shown},
  names::{self, current_branch, revision, upstream_remote},
  read::in_progress,
};
use crate::{app_mcp::ToolError, messages::Message};

/// The remote a branch without an upstream pushes to and fetches from.
const ORIGIN: &str = "origin";

/// The git a write runs, local and over the network, and the commands it ran with what git last said.
pub(super) struct Ran<'a> {
  pub(super) local: &'a Git<'a>,
  pub(super) network: &'a Git<'a>,
  pub(super) commands: Vec<String>,
  pub(super) said: Option<String>,
}

/// Whether a command works in the repository or reaches a remote.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reach {
  Local,
  Network,
}

impl Ran<'_> {
  /// Runs `git <args>` with `input`, recorded; a failure is refused as git said.
  pub(super) fn run(&mut self, reach: Reach, args: &[&str], input: Option<&str>) -> Result<String, ToolError> {
    let git = match reach {
      Reach::Local => self.local,
      Reach::Network => self.network,
    };
    self.commands.push(shown(args));
    let output = git.run(args, input)?;
    if !output.success() {
      return Err(refusal(&output, args));
    }
    self.said = Some(said_of_success(&output));
    Ok(output.stdout)
  }

  pub(super) fn done(&self) -> Result<Value, ToolError> {
    Ok(json!({
      "done": true,
      "ran": self.commands,
      "said": self.said,
      "branch": current_branch(self.local)?,
      "head": names::commit_of(self.local, "HEAD")?,
    }))
  }
}

/// `git_commit`: the named paths, every change, or what is staged, with the message; the author is git's own setting.
/// Files still in conflict are refused: adding them would commit their conflict markers.
pub(super) fn commit(ran: &mut Ran, asked: &Commit) -> Result<Value, ToolError> {
  if asked.message.trim().is_empty() {
    return Err(ToolError::InvalidArguments("message: required".to_owned()));
  }
  let paths = names::paths("paths", &asked.paths)?;
  let unmerged = conflicted_files(ran.local)?;
  if !unmerged.is_empty() {
    return Err(refused(
      "conflict",
      Message::new("orchestrator.git.unresolved").with("files", unmerged.join(", ")),
    ));
  }
  let changed = if !paths.is_empty() || asked.all {
    let mut args = vec!["status", "--porcelain", "--"];
    args.extend(&paths);
    !ran.local.ok(&args)?.trim().is_empty()
  } else {
    !ran.local.succeeds(&["diff", "--cached", "--quiet"])?
  };
  if !changed {
    return Err(refused(
      "nothing_to_commit",
      Message::new("orchestrator.git.nothingToCommit"),
    ));
  }
  let message = Some(asked.message.as_str());
  if !paths.is_empty() {
    let mut add = vec!["add", "-A", "--"];
    add.extend(&paths);
    ran.run(Reach::Local, &add, None)?;
    let mut commit = vec!["commit", "-F", "-", "--"];
    commit.extend(&paths);
    ran.run(Reach::Local, &commit, message)?;
  } else {
    if asked.all {
      ran.run(Reach::Local, &["add", "-A"], None)?;
    }
    ran.run(Reach::Local, &["commit", "-F", "-"], message)?;
  }
  ran.done()
}

/// `git_create_branch`: a new branch at HEAD or `start`, switched to when asked.
pub(super) fn create_branch(ran: &mut Ran, asked: &CreateBranch) -> Result<Value, ToolError> {
  let name = names::branch(ran.local, "name", &asked.name)?;
  let mut args = if asked.switch {
    not_in_progress(ran.local)?;
    vec!["switch", "-c", name]
  } else {
    vec!["branch", name]
  };
  if let Some(start) = &asked.start {
    args.push(names::start(ran.local, "start", start)?);
  }
  ran.run(Reach::Local, &args, None)?;
  ran.done()
}

/// `git_switch`: only as git switches, keeping every local change; one it would overwrite is refused.
pub(super) fn switch(ran: &mut Ran, asked: &Switch) -> Result<Value, ToolError> {
  let branch = names::branch(ran.local, "branch", &asked.branch)?;
  not_in_progress(ran.local)?;
  ran.run(Reach::Local, &["switch", branch], None)?;
  ran.done()
}

/// `git_pull`: fast-forward only, unless the person asked for a merge; a merge's conflict is undone. The remote is one
/// the repository lists and the branch a plain name: the refspec is built here, never taken from the chat.
pub(super) fn pull(ran: &mut Ran, asked: &Pull) -> Result<Value, ToolError> {
  not_in_progress(ran.local)?;
  let mut args = if asked.merge {
    vec!["pull", "--no-rebase", "--no-edit", "--end-of-options"]
  } else {
    vec!["pull", "--ff-only", "--end-of-options"]
  };
  let refspec;
  if let Some(remote) = &asked.remote {
    args.push(names::remote(ran.local, "remote", remote)?);
    if let Some(branch) = &asked.branch {
      refspec = format!("refs/heads/{}", names::branch(ran.local, "branch", branch)?);
      args.push(&refspec);
    }
  } else if asked.branch.is_some() {
    return Err(ToolError::InvalidArguments("branch: needs remote".to_owned()));
  }
  match ran.run(Reach::Network, &args, None) {
    Ok(_) => ran.done(),
    Err(error) => Err(conflicted(ran, error, (&args, &["merge", "--abort"]))),
  }
}

/// `git_push`: a branch to the remote's branch of the same name, never forced; a rejection is told in git's words.
pub(super) fn push(ran: &mut Ran, asked: &Push) -> Result<Value, ToolError> {
  let branch = match &asked.branch {
    Some(branch) => names::branch(ran.local, "branch", branch)?.to_owned(),
    None => {
      current_branch(ran.local)?.ok_or_else(|| refused("detached_head", Message::new("orchestrator.git.detached")))?
    }
  };
  let remote = remote_of(ran.local, asked.remote.as_deref(), &branch)?;
  let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
  let mut args = vec!["push"];
  if asked.set_upstream {
    args.push("-u");
  }
  args.extend(["--end-of-options", remote.as_str(), refspec.as_str()]);
  ran.run(Reach::Network, &args, None)?;
  ran.done()
}

/// `git_merge`: a branch into the current one; a conflict is undone.
pub(super) fn merge(ran: &mut Ran, asked: &Merge) -> Result<Value, ToolError> {
  not_in_progress(ran.local)?;
  let mut args = vec!["merge", "--no-edit"];
  if asked.no_ff {
    args.push("--no-ff");
  }
  if let Some(message) = &asked.message {
    args.extend(["-m", message]);
  }
  args.extend(["--end-of-options", names::branch(ran.local, "branch", &asked.branch)?]);
  match ran.run(Reach::Local, &args, None) {
    Ok(_) => ran.done(),
    Err(error) => Err(conflicted(ran, error, (&args, &["merge", "--abort"]))),
  }
}

/// `git_revert`: a new commit that undoes one; a conflict is undone.
pub(super) fn revert(ran: &mut Ran, asked: &Revert) -> Result<Value, ToolError> {
  not_in_progress(ran.local)?;
  let mainline = asked.mainline.map(|parent| parent.to_string());
  let mut args = vec!["revert", "--no-edit"];
  if let Some(mainline) = &mainline {
    args.extend(["-m", mainline]);
  }
  args.extend(["--end-of-options", revision("commit", &asked.commit)?]);
  match ran.run(Reach::Local, &args, None) {
    Ok(_) => ran.done(),
    Err(error) => Err(conflicted(ran, error, (&args, &["revert", "--abort"]))),
  }
}

/// `git_tag`: a tag at HEAD or a start point, annotated with a message.
pub(super) fn tag(ran: &mut Ran, asked: &Tag) -> Result<Value, ToolError> {
  let name = names::tag(ran.local, "name", &asked.name)?;
  let mut args = vec!["tag"];
  if asked.message.is_some() {
    args.extend(["-a", "-F", "-"]);
  }
  args.push(name);
  if let Some(rev) = &asked.rev {
    args.push(names::start(ran.local, "rev", rev)?);
  }
  ran.run(Reach::Local, &args, asked.message.as_deref())?;
  ran.done()
}

/// `git_fetch`: a remote's or every remote's branches and tags; with `prune`, remote-tracking branches the remote no
/// longer has go, and no local branch is touched.
pub(super) fn fetch(ran: &mut Ran, asked: &Fetch) -> Result<Value, ToolError> {
  let remote;
  let mut args = vec!["fetch"];
  if asked.prune {
    args.push("--prune");
  }
  if asked.all {
    args.push("--all");
  } else {
    let branch = current_branch(ran.local)?.unwrap_or_default();
    remote = remote_of(ran.local, asked.remote.as_deref(), &branch)?;
    args.extend(["--end-of-options", remote.as_str()]);
  }
  ran.run(Reach::Network, &args, None)?;
  ran.done()
}

/// The remote named, else `branch`'s upstream remote, else `origin`: always one the repository lists.
pub(super) fn remote_of(git: &Git, named: Option<&str>, branch: &str) -> Result<String, ToolError> {
  let chosen = match named {
    Some(named) => named.to_owned(),
    None => upstream_remote(git, branch)?.unwrap_or_else(|| ORIGIN.to_owned()),
  };
  Ok(names::remote(git, "remote", &chosen)?.to_owned())
}

/// The files git holds in conflict.
fn conflicted_files(git: &Git) -> Result<Vec<String>, ToolError> {
  Ok(
    git
      .ok(&["diff", "--name-only", "--diff-filter=U"])?
      .lines()
      .map(str::to_owned)
      .collect(),
  )
}

/// Refused while a merge, rebase, cherry-pick or revert is in progress: the person finishes or aborts it first.
pub(super) fn not_in_progress(git: &Git) -> Result<(), ToolError> {
  match in_progress(git)? {
    None => Ok(()),
    Some(operation) => Err(refused(
      "operation_in_progress",
      Message::new("orchestrator.git.inProgress").with("operation", operation.to_owned()),
    )),
  }
}

/// The refusal of a merge, pull, revert or rebase that failed as `error`: when it stopped at conflicts, they are
/// undone with `abort` and refused as `conflict`, naming the files; any other failure as it was.
pub(super) fn conflicted(ran: &mut Ran, error: ToolError, (args, abort): (&[&str], &[&str])) -> ToolError {
  // Conflicts that cannot be read are not told as conflicts: the command's own refusal says what git said.
  let files = conflicted_files(ran.local).unwrap_or_default();
  if files.is_empty() {
    return error;
  }
  let (command, files) = (shown(args), files.join(", "));
  if ran.run(Reach::Local, abort, None).is_err() {
    return refused(
      "conflict",
      Message::new("orchestrator.git.conflictNotUndone")
        .with("command", command)
        .with("files", files)
        .with("abort", shown(abort)),
    );
  }
  refused(
    "conflict",
    Message::new("orchestrator.git.conflictUndone")
      .with("command", command)
      .with("files", files),
  )
}
