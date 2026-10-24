//! What can lose work is only ever proposed (`git_propose`), whatever the person's switch says, and only a person
//! applies it: a reset, a clean, a branch's or tag's deletion, a forced push, discarding changes, a switch that
//! discards them, a rebase and aborting a merge. The proposal reads what it would lose (commits no other branch holds,
//! uncommitted changes, untracked files in the way, files) and what it stands on — the commits its refs point at, the
//! branch HEAD is on and a fingerprint of every uncommitted change ([`survey`]); applying it reads both again and
//! refuses (`changed_since_proposed`) once either changed, so the person's click does only what the card showed. A
//! forced push or a remote deletion leases exactly the remote commit the card was read with (`--force-with-lease`).
//! Orchester's `.orchester/` is never cleaned.

mod commands;
mod survey;
mod work;

use orchester_core::orchestrator::git::{GitChange, GitOperation, GitPlace, GitProposal, ResetMode};
use serde_json::Value;

use self::{commands::commands, survey::survey};
use super::{
  args::{Mode, Operation},
  command::{Git, refused, shown},
  names::{self, current_branch, revision},
  write::{Ran, conflicted, remote_of},
};
use crate::{app_mcp::ToolError, messages::Message};

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
        GitProposal::Rebase { .. } => conflicted(ran, error, (&args, &["rebase", "--abort"])),
        _ => error,
      });
    }
  }
  ran.done()
}

/// The proposal of `operation`, its names checked ([`names`]: plain branch and tag names, remotes the repository
/// lists, paths inside it) and its remote and branch filled in where the agent left them out.
fn proposal(git: &Git, operation: Operation) -> Result<GitProposal, ToolError> {
  let owned = |name: Result<&str, ToolError>| name.map(str::to_owned);
  let remote = |remote: Option<String>| {
    remote
      .map(|remote| owned(names::remote(git, "remote", &remote)))
      .transpose()
  };
  let paths = |named: &[String]| -> Result<Vec<String>, ToolError> {
    Ok(names::paths("paths", named)?.into_iter().map(str::to_owned).collect())
  };
  Ok(match operation {
    Operation::Reset { to, mode } => GitProposal::Reset {
      to: owned(revision("to", &to))?,
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
      paths: paths(&named)?,
      directories,
      ignored,
    },
    Operation::DeleteBranch { branch, remote: named } => GitProposal::DeleteBranch {
      branch: owned(names::branch(git, "branch", &branch))?,
      remote: remote(named)?,
    },
    Operation::ForcePush { remote, branch } => {
      let branch = match branch {
        Some(branch) => owned(names::branch(git, "branch", &branch))?,
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
    Operation::DiscardChanges { paths: named } => GitProposal::DiscardChanges { paths: paths(&named)? },
    Operation::SwitchDiscarding { branch } => GitProposal::SwitchDiscarding {
      branch: owned(names::branch(git, "branch", &branch))?,
    },
    Operation::Rebase { onto } => GitProposal::Rebase {
      onto: owned(revision("onto", &onto))?,
    },
    Operation::DeleteTag { tag, remote: named } => GitProposal::DeleteTag {
      tag: owned(names::tag(git, "tag", &tag))?,
      remote: remote(named)?,
    },
    Operation::AbortMerge => GitProposal::AbortMerge,
  })
}

/// Whether applying `proposal` changes the working tree or the index, so it holds the local-integration lease while it
/// runs: all but deleting a branch or a tag and a forced push, which change refs or a remote only.
pub(super) const fn changes_checkout(proposal: &GitProposal) -> bool {
  !matches!(
    proposal,
    GitProposal::DeleteBranch { .. } | GitProposal::DeleteTag { .. } | GitProposal::ForcePush { .. }
  )
}
