//! The commands that carry a proposal out, built from what the card was read with: a reset or rebase goes to the
//! commit the card read, a forced push or a remote deletion leases the remote commit it read, and a clean never touches
//! Orchester's `.orchester/`.

use orchester_core::orchestrator::git::{GitProposal, ResetMode};

use super::{super::write::Reach, survey::Survey};

/// What a clean always keeps: Orchester's own folder, also with `-x`, which drops the ignore rules but `-e`'s.
const PROJECT_DATA: &str = "/.orchester/";

/// The commands that carry `proposal` out, standing on `survey`.
pub(super) fn commands(proposal: &GitProposal, survey: &Survey) -> Vec<(Reach, Vec<String>)> {
  let owned = |args: &[&str]| args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
  let lease = |target: &str, read: &str| format!("--force-with-lease={target}:{}", survey.at(read));
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
    GitProposal::DeleteBranch { branch, remote: None } => {
      vec![(Reach::Local, owned(&["branch", "-D", "--end-of-options", branch]))]
    }
    GitProposal::DeleteBranch {
      branch,
      remote: Some(remote),
    } => {
      let target = format!("refs/heads/{branch}");
      let lease = lease(&target, &format!("refs/remotes/{remote}/{branch}"));
      vec![(
        Reach::Network,
        owned(&["push", &lease, "--delete", "--end-of-options", remote, &target]),
      )]
    }
    GitProposal::ForcePush { remote, branch } => {
      let target = format!("refs/heads/{branch}");
      let lease = lease(&target, &format!("refs/remotes/{remote}/{branch}"));
      let refspec = format!("{target}:{target}");
      vec![(
        Reach::Network,
        owned(&["push", &lease, "--end-of-options", remote, &refspec]),
      )]
    }
    GitProposal::DiscardChanges { paths } => {
      let mut args = owned(&["restore", "--source=HEAD", "--staged", "--worktree", "--"]);
      args.extend(paths.iter().cloned());
      vec![(Reach::Local, args)]
    }
    GitProposal::SwitchDiscarding { branch } => {
      vec![(Reach::Local, owned(&["switch", "--discard-changes", branch]))]
    }
    GitProposal::Rebase { .. } => vec![(Reach::Local, owned(&["rebase", survey.at("onto")]))],
    GitProposal::DeleteTag { tag, remote: None } => vec![(Reach::Local, owned(&["tag", "-d", tag]))],
    GitProposal::DeleteTag {
      tag,
      remote: Some(remote),
    } => {
      let target = format!("refs/tags/{tag}");
      let lease = lease(&target, &target);
      vec![(
        Reach::Network,
        owned(&["push", &lease, "--delete", "--end-of-options", remote, &target]),
      )]
    }
    GitProposal::AbortMerge => vec![(Reach::Local, owned(&["merge", "--abort"]))],
  }
}

/// `git clean`'s arguments: a dry run (`-n`) that only lists, or the clean itself (`-f`), never of `.orchester/`.
pub(super) fn clean_args(paths: &[String], directories: bool, ignored: bool, dry: bool) -> Vec<String> {
  let mut args = vec![
    "clean".to_owned(),
    if dry { "-n" } else { "-f" }.to_owned(),
    "-e".to_owned(),
    PROJECT_DATA.to_owned(),
  ];
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
