//! What a proposal stands on and what it would lose, as git reads it: the commits its refs point at, the branch HEAD is
//! on and a fingerprint of every uncommitted change and untracked file, read once when the card is made and again
//! when it is applied.

use std::collections::BTreeSet;

use orchester_core::orchestrator::git::{GitProposal, ResetMode};

use super::{
  super::{
    command::{Git, refused},
    names::commit_of,
    read::in_progress,
  },
  commands::clean_args,
};
use crate::{app_mcp::ToolError, messages::Message};

/// What a proposal stands on, `name=value` per line, and what it would lose, one per line.
#[derive(PartialEq, Eq)]
pub(super) struct Survey {
  basis: Vec<(String, String)>,
  pub(super) loses: Vec<String>,
}

impl Survey {
  pub(super) fn basis_text(&self) -> String {
    self
      .basis
      .iter()
      .map(|(name, value)| format!("{name}={value}"))
      .collect::<Vec<_>>()
      .join("\n")
  }

  /// What `name` stood at; empty for a ref that does not exist.
  pub(super) fn at(&self, name: &str) -> &str {
    self
      .basis
      .iter()
      .find(|(found, _)| found == name)
      .map_or("", |(_, value)| value.as_str())
  }
}

/// What `proposal` stands on and would lose, as git reads it now.
pub(super) fn survey(git: &Git, proposal: &GitProposal) -> Result<Survey, ToolError> {
  let mut reading = Reading::of(git)?;
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
      let mut loses = changes(git, &[])?;
      loses.extend(in_the_way(git, branch)?);
      loses
    }
    GitProposal::Rebase { onto } => {
      reading.known("onto", onto)?;
      commits(git, &["HEAD", "--not", onto])?
    }
    GitProposal::DeleteTag { tag, .. } => {
      let target = format!("refs/tags/{tag}");
      let object = reading.ref_value(&target)?;
      vec![format!("{tag} {object}")]
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

/// The refs a proposal stands on, read one by one, after HEAD, the branch it is on and the uncommitted work.
struct Reading<'a> {
  git: &'a Git<'a>,
  basis: Vec<(String, String)>,
}

impl<'a> Reading<'a> {
  fn of(git: &'a Git<'a>) -> Result<Self, ToolError> {
    let head = commit_of(git, "HEAD")?.unwrap_or_default();
    let on = git.run(&["symbolic-ref", "--quiet", "HEAD"], None)?;
    let on = if on.success() {
      on.stdout.trim().to_owned()
    } else {
      "detached".to_owned()
    };
    let work = fingerprint(git, !head.is_empty())?;
    Ok(Self {
      git,
      basis: vec![
        ("HEAD".to_owned(), head),
        ("HEAD-ref".to_owned(), on),
        ("work".to_owned(), work),
      ],
    })
  }

  /// The commit `rev` stands at, kept as `name`; empty for none.
  fn stands(&mut self, name: &str, rev: &str) -> Result<String, ToolError> {
    let commit = commit_of(self.git, rev)?.unwrap_or_default();
    self.basis.push((name.to_owned(), commit.clone()));
    Ok(commit)
  }

  /// [`Self::stands`] of `rev`, kept as `name`; refused when `rev` names no commit.
  fn known(&mut self, name: &str, rev: &str) -> Result<(), ToolError> {
    if self.stands(name, rev)?.is_empty() {
      return Err(unknown(rev));
    }
    Ok(())
  }

  /// The object the ref `target` holds (a tag's own object for an annotated tag), kept under its name; refused when
  /// there is no such ref.
  fn ref_value(&mut self, target: &str) -> Result<String, ToolError> {
    let output = self
      .git
      .run(&["rev-parse", "--verify", "--quiet", "--end-of-options", target], None)?;
    let object = output.stdout.trim().to_owned();
    if !output.success() || object.is_empty() {
      return Err(unknown(target));
    }
    self.basis.push((target.to_owned(), object.clone()));
    Ok(object)
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

/// A fingerprint of the uncommitted work: git's hash of every change to tracked files against HEAD (`with_head`), or
/// to the index without a commit, and of the untracked files' names. An edit since the card was read changes it.
fn fingerprint(git: &Git, with_head: bool) -> Result<String, ToolError> {
  let diff = if with_head {
    git.ok(&["diff", "HEAD", "--binary", "--no-ext-diff", "--no-textconv"])?
  } else {
    git.ok(&["diff", "--cached", "--binary", "--no-ext-diff", "--no-textconv"])?
  };
  let untracked = git.ok(&["ls-files", "--others", "--exclude-standard", "-z"])?;
  let work = format!("{diff}\0{untracked}");
  Ok(git.ok_with(&["hash-object", "--stdin"], Some(&work))?.trim().to_owned())
}

/// The untracked files, ignored ones among them, that switching to `branch` with `--discard-changes` would overwrite:
/// those the branch's tree holds at the same path, as `untracked\t<path>`.
fn in_the_way(git: &Git, branch: &str) -> Result<Vec<String>, ToolError> {
  let tree: BTreeSet<String> = git
    .ok(&["ls-tree", "-r", "--name-only", "-z", "--end-of-options", branch])?
    .split('\0')
    .filter(|path| !path.is_empty())
    .map(str::to_owned)
    .collect();
  let untracked = git.ok(&["ls-files", "--others", "--directory", "-z"])?;
  let mut hit = BTreeSet::new();
  for entry in untracked.split('\0').filter(|entry| !entry.is_empty()) {
    if let Some(folder) = entry.strip_suffix('/') {
      let inside = format!("{folder}/");
      hit.extend(tree.iter().filter(|path| path.starts_with(&inside)).cloned());
    } else if tree.contains(entry) {
      hit.insert(entry.to_owned());
    }
  }
  Ok(hit.into_iter().map(|path| format!("untracked\t{path}")).collect())
}

/// The files a dry run of `git clean` lists.
fn cleaned(git: &Git, args: &[String]) -> Result<Vec<String>, ToolError> {
  let args: Vec<&str> = args.iter().map(String::as_str).collect();
  let files: Vec<String> = git
    .ok(&args)?
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

/// The refusal of a revision or ref git finds nothing for.
fn unknown(named: &str) -> ToolError {
  refused(
    "git_failed",
    Message::new("orchestrator.git.unknownRevision").with("revision", named.to_owned()),
  )
}
