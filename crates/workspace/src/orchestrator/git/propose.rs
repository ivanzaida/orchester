//! What can lose work is only ever proposed (`git_propose`), whatever the person's switch says, and only a person
//! applies it: a reset, a clean, a branch's or tag's deletion, a forced push, discarding changes, a switch that
//! discards them, a rebase and aborting a merge. The proposal reads what it would lose (commits no other branch holds,
//! uncommitted changes, files) and the commits its refs point at; applying it checks both again and refuses
//! (`changed_since_proposed`) once either moved, so the person's click does only what the card showed. A forced
//! push or a remote deletion leases exactly the remote commit the card was read with (`--force-with-lease`).

use orchester_core::orchestrator::git::{GitChange, GitOperation, GitPlace, GitProposal, ResetMode};
use serde_json::Value;

use super::{
  args::{Mode, Operation},
  command::{Git, refused, shown},
  names::{commit_of, current_branch, paths, revision},
  read::in_progress,
  write::{Ran, Reach, conflicted, remote_of},
};
use crate::{app_mcp::ToolError, messages::Message};

/// What a proposal stands on, `name=commit` per ref, and what it would lose, one per line.
#[derive(PartialEq, Eq)]
struct Survey {
  basis: Vec<(String, String)>,
  loses: Vec<String>,
}

impl Survey {
  fn basis_text(&self) -> String {
    self
      .basis
      .iter()
      .map(|(name, commit)| format!("{name}={commit}"))
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// The commit `name` stood at; empty for one that does not exist.
  fn at(&self, name: &str) -> &str {
    self
      .basis
      .iter()
      .find(|(found, _)| found == name)
      .map_or("", |(_, commit)| commit.as_str())
  }
}

/// The proposal the agent's `operation` names at `place`, with what it would run and lose; refused when there is
/// nothing to do. Changes nothing.
pub(super) fn propose(git: &Git, operation: Operation, place: GitPlace) -> Result<GitChange, ToolError> {
  let proposal = proposal(git, operation)?;
  let survey = survey(git, &proposal)?;
  Ok(GitChange {
    place,
    commands: commands(&proposal, &survey)
      .iter()
      .map(|(_, args)| shown(&args.iter().map(String::as_str).collect::<Vec<_>>()))
      .collect(),
    operation: GitOperation::Propose(proposal),
    said: None,
    basis: survey.basis_text(),
    loses: survey.loses,
  })
}

/// Applies the proposal `change`, once what it stands on and would lose is as the card showed; a rebase that stops at
/// conflicts is undone. Blocks on git.
pub(super) fn apply(ran: &mut Ran, change: &GitChange) -> Result<Value, ToolError> {
  let GitOperation::Propose(proposal) = &change.operation else {
    return Err(ToolError::InvalidArguments("not a proposed git operation".to_owned()));
  };
  let survey = survey(ran.local, proposal)?;
  if survey.basis_text() != change.basis || survey.loses != change.loses {
    return Err(refused(
      "changed_since_proposed",
      Message::new("orchestrator.git.changed"),
    ));
  }
  for (reach, args) in commands(proposal, &survey) {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let Err(error) = ran.run(reach, &args, None) {
      return Err(match proposal {
        GitProposal::Rebase { .. } => conflicted(ran, error, (&args, &["rebase", "--abort"]), false),
        _ => error,
      });
    }
  }
  ran.done()
}

/// The proposal of `operation`, its remote and branch filled in where the agent left them out.
fn proposal(git: &Git, operation: Operation) -> Result<GitProposal, ToolError> {
  let text = |argument: &str, value: &str| revision(argument, value).map(str::to_owned);
  Ok(match operation {
    Operation::Reset { to, mode } => GitProposal::Reset {
      to: text("to", &to)?,
      mode: match mode {
        Mode::Hard => ResetMode::Hard,
        Mode::Mixed => ResetMode::Mixed,
        Mode::Soft => ResetMode::Soft,
      },
    },
    Operation::Clean {
      paths: named,
      directories,
      ignored,
    } => GitProposal::Clean {
      paths: paths(&named)?.into_iter().map(str::to_owned).collect(),
      directories,
      ignored,
    },
    Operation::DeleteBranch { branch, remote } => GitProposal::DeleteBranch {
      branch: text("branch", &branch)?,
      remote: remote.map(|remote| text("remote", &remote)).transpose()?,
    },
    Operation::ForcePush { remote, branch } => {
      let branch = match branch {
        Some(branch) => text("branch", &branch)?,
        None => {
          current_branch(git)?.ok_or_else(|| refused("detached_head", Message::new("orchestrator.git.detached")))?
        }
      };
      GitProposal::ForcePush {
        remote: remote_of(git, remote.as_deref(), &branch)?,
        branch,
      }
    }
    Operation::DiscardChanges { paths: named } if named.is_empty() => {
      return Err(ToolError::InvalidArguments("paths: name the files".to_owned()));
    }
    Operation::DiscardChanges { paths: named } => GitProposal::DiscardChanges {
      paths: paths(&named)?.into_iter().map(str::to_owned).collect(),
    },
    Operation::SwitchDiscarding { branch } => GitProposal::SwitchDiscarding {
      branch: text("branch", &branch)?,
    },
    Operation::Rebase { onto } => GitProposal::Rebase {
      onto: text("onto", &onto)?,
    },
    Operation::DeleteTag { tag, remote } => GitProposal::DeleteTag {
      tag: text("tag", &tag)?,
      remote: remote.map(|remote| text("remote", &remote)).transpose()?,
    },
    Operation::AbortMerge => GitProposal::AbortMerge,
  })
}

/// What `proposal` stands on and would lose, as git reads it now.
fn survey(git: &Git, proposal: &GitProposal) -> Result<Survey, ToolError> {
  let mut reading = Reading {
    git,
    basis: vec![("HEAD".to_owned(), commit_of(git, "HEAD")?.unwrap_or_default())],
  };
  let loses = match proposal {
    GitProposal::Reset { to, mode } => {
      reading.known("to", to)?;
      let mut loses = commits(git, &["HEAD", "--not", to])?;
      if *mode == ResetMode::Hard {
        loses.extend(changes(git, &[])?);
      }
      loses
    }
    GitProposal::Clean {
      paths,
      directories,
      ignored,
    } => cleaned(git, &clean_args(paths, *directories, *ignored, true))?,
    GitProposal::DeleteBranch { branch, remote } => reading.branch_loses(branch, remote.as_deref())?,
    GitProposal::ForcePush { remote, branch } => reading.pushed_over(remote, branch)?,
    GitProposal::DiscardChanges { paths } => changes(git, paths)?,
    GitProposal::SwitchDiscarding { branch } => {
      reading.known("branch", branch)?;
      changes(git, &[])?
    }
    GitProposal::Rebase { onto } => {
      reading.known("onto", onto)?;
      commits(git, &["HEAD", "--not", onto])?
    }
    GitProposal::DeleteTag { tag, .. } => {
      let local = format!("refs/tags/{tag}");
      let commit = reading.stands(&local, &local)?;
      vec![format!("{tag} {commit}")]
    }
    GitProposal::AbortMerge => {
      if in_progress(git)? != Some("merge") {
        return Err(refused("nothing_in_progress", Message::new("orchestrator.git.noMerge")));
      }
      changes(git, &[])?
    }
  };
  Ok(Survey {
    basis: reading.basis,
    loses,
  })
}

/// The refs a proposal stands on, read one by one.
struct Reading<'a> {
  git: &'a Git<'a>,
  basis: Vec<(String, String)>,
}

impl Reading<'_> {
  /// The commit `rev` stands at, kept as `name`; empty for none.
  fn stands(&mut self, name: &str, rev: &str) -> Result<String, ToolError> {
    let commit = commit_of(self.git, rev)?.unwrap_or_default();
    self.basis.push((name.to_owned(), commit.clone()));
    Ok(commit)
  }

  /// [`Self::stands`] of `rev`, kept as `name`; refused when `rev` names no commit.
  fn known(&mut self, name: &str, rev: &str) -> Result<(), ToolError> {
    if self.stands(name, rev)?.is_empty() {
      return Err(refused(
        "git_failed",
        Message::new("orchestrator.git.unknownRevision").with("revision", rev.to_owned()),
      ));
    }
    Ok(())
  }

  /// What deleting `branch` (here, or with `remote` the remote's) loses: its commits no other branch holds.
  fn branch_loses(&mut self, branch: &str, remote: Option<&str>) -> Result<Vec<String>, ToolError> {
    let (rev, named) = match remote {
      None => (format!("refs/heads/{branch}"), branch.to_owned()),
      Some(remote) => (format!("refs/remotes/{remote}/{branch}"), format!("{remote}/{branch}")),
    };
    self.known(&rev, &rev)?;
    let exclude = format!("--exclude={named}");
    let others: [&str; 3] = match remote {
      None => [exclude.as_str(), "--branches", "--remotes"],
      Some(_) => ["--branches", exclude.as_str(), "--remotes"],
    };
    let mut revisions = vec![rev.as_str(), "--not"];
    revisions.extend(others);
    commits(self.git, &revisions)
  }

  /// What pushing `branch` over `remote`'s loses: the remote's commits the branch does not hold.
  fn pushed_over(&mut self, remote: &str, branch: &str) -> Result<Vec<String>, ToolError> {
    let local = format!("refs/heads/{branch}");
    self.known(&local, &local)?;
    let tracking = format!("refs/remotes/{remote}/{branch}");
    if self.stands(&tracking, &tracking)?.is_empty() {
      return Ok(Vec::new());
    }
    commits(self.git, &[&tracking, "--not", &local])
  }
}

/// The commands that carry `proposal` out, standing on `survey`.
fn commands(proposal: &GitProposal, survey: &Survey) -> Vec<(Reach, Vec<String>)> {
  let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
  let lease = |branch: &str, tracking: &str| format!("--force-with-lease=refs/heads/{branch}:{}", survey.at(tracking));
  match proposal {
    // The commit the card showed, not the name: a name that moves since is refused before this runs.
    GitProposal::Reset { mode, .. } => {
      let mode = match mode {
        ResetMode::Hard => "--hard",
        ResetMode::Mixed => "--mixed",
        ResetMode::Soft => "--soft",
      };
      vec![(Reach::Local, owned(&["reset", mode, survey.at("to")]))]
    }
    GitProposal::Clean {
      paths,
      directories,
      ignored,
    } => vec![(Reach::Local, clean_args(paths, *directories, *ignored, false))],
    GitProposal::DeleteBranch { branch, remote: None } => vec![(Reach::Local, owned(&["branch", "-D", branch]))],
    GitProposal::DeleteBranch {
      branch,
      remote: Some(remote),
    } => {
      let lease = lease(branch, &format!("refs/remotes/{remote}/{branch}"));
      vec![(
        Reach::Network,
        owned(&["push", &lease, remote, "--delete", &format!("refs/heads/{branch}")]),
      )]
    }
    GitProposal::ForcePush { remote, branch } => {
      let lease = lease(branch, &format!("refs/remotes/{remote}/{branch}"));
      let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
      vec![(Reach::Network, owned(&["push", &lease, remote, &refspec]))]
    }
    GitProposal::DiscardChanges { paths } => {
      let mut args = owned(&["restore", "--source=HEAD", "--staged", "--worktree", "--"]);
      args.extend(paths.iter().cloned());
      vec![(Reach::Local, args)]
    }
    GitProposal::SwitchDiscarding { branch } => vec![(Reach::Local, owned(&["switch", "--discard-changes", branch]))],
    GitProposal::Rebase { onto } => vec![(Reach::Local, owned(&["rebase", onto]))],
    GitProposal::DeleteTag { tag, remote: None } => vec![(Reach::Local, owned(&["tag", "-d", tag]))],
    GitProposal::DeleteTag {
      tag,
      remote: Some(remote),
    } => vec![(
      Reach::Network,
      owned(&["push", remote, "--delete", &format!("refs/tags/{tag}")]),
    )],
    GitProposal::AbortMerge => vec![(Reach::Local, owned(&["merge", "--abort"]))],
  }
}

/// `git clean`'s arguments: a dry run (`-n`) that only lists, or the clean itself (`-f`).
fn clean_args(paths: &[String], directories: bool, ignored: bool, dry: bool) -> Vec<String> {
  let mut args = vec!["clean".to_owned(), if dry { "-n" } else { "-f" }.to_owned()];
  if directories {
    args.push("-d".to_owned());
  }
  if ignored {
    args.push("-x".to_owned());
  }
  args.push("--".to_owned());
  args.extend(paths.iter().cloned());
  args
}

/// The files a dry run of `git clean` lists.
fn cleaned(git: &Git, args: &[String]) -> Result<Vec<String>, ToolError> {
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let text = git.ok(&args)?;
  let files: Vec<String> = text
    .lines()
    .filter_map(|line| line.strip_prefix("Would remove "))
    .map(str::to_owned)
    .collect();
  if files.is_empty() {
    return Err(refused(
      "nothing_to_clean",
      Message::new("orchestrator.git.nothingToClean"),
    ));
  }
  Ok(files)
}

/// The commits `git log <revisions>` lists, as `<short> <subject>`.
fn commits(git: &Git, revisions: &[&str]) -> Result<Vec<String>, ToolError> {
  let mut args = vec!["log", "--format=%h %s"];
  args.extend(revisions);
  Ok(git.ok(&args)?.lines().map(str::to_owned).collect())
}

/// The uncommitted changes to tracked files under `paths` (all of them without), as `<status>\t<path>`.
fn changes(git: &Git, paths: &[String]) -> Result<Vec<String>, ToolError> {
  let mut args = vec!["diff", "HEAD", "--name-status", "--"];
  args.extend(paths.iter().map(String::as_str));
  Ok(git.ok(&args)?.lines().map(str::to_owned).collect())
}
