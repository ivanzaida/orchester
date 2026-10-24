//! The chat's git reads, always open: the working tree's state, history, differences, a commit or a file at a
//! revision, branches with how far each is from its upstream, who last changed lines, and the remotes. Each runs one
//! read of git's and changes nothing (git takes no optional locks). Long output is cut and the answer says so.

use std::path::Path;

use orchester_core::repository::redact_userinfo;
use orchester_exec::programs::Programs;
use serde_json::{Value, json};

use super::{
  args::{Blame, Diff, Log, Nothing, Show},
  command::Git,
  names::{self, revision},
};
use crate::app_mcp::ToolError;

/// The commits `git_log` answers with when the call names no limit, and the most it answers with.
const LOG_LIMIT: u32 = 30;
const LOG_MAX: u32 = 200;

/// `git_status`: the branch, its upstream and how far apart they are, a merge or rebase in progress, and every
/// changed, conflicted or untracked file.
pub(super) fn status(git: &Git, _: &Nothing) -> Result<Value, ToolError> {
  let text = git.ok(&["status", "--porcelain=v2", "--branch", "-z"])?;
  let mut status = Status::default();
  let mut entries = text.split('\0').filter(|entry| !entry.is_empty());
  while let Some(entry) = entries.next() {
    status.read(entry, &mut entries);
  }
  Ok(json!({
    "branch": status.branch,
    "head": status.head,
    "upstream": status.upstream,
    "ahead": status.ahead,
    "behind": status.behind,
    "in_progress": in_progress(git)?,
    "clean": status.files.is_empty(),
    "files": status.files,
  }))
}

/// What `git status --porcelain=v2 --branch -z` told.
#[derive(Default)]
struct Status {
  branch: Option<String>,
  head: Option<String>,
  upstream: Option<String>,
  ahead: u32,
  behind: u32,
  files: Vec<Value>,
}

impl Status {
  fn read<'a>(&mut self, entry: &str, rest: &mut impl Iterator<Item = &'a str>) {
    if let Some(header) = entry.strip_prefix("# ") {
      self.header(header);
      return;
    }
    let file = match entry.split_at_checked(2) {
      Some(("1 ", fields)) => fields.splitn(8, ' ').collect::<Vec<_>>().split_last().map(|(path, fields)| {
        json!({ "path": path, "staged": state(fields[0], 0), "unstaged": state(fields[0], 1) })
      }),
      Some(("2 ", fields)) => fields.splitn(9, ' ').collect::<Vec<_>>().split_last().map(|(path, fields)| {
        json!({ "path": path, "from": rest.next(), "staged": state(fields[0], 0), "unstaged": state(fields[0], 1) })
      }),
      Some(("u ", fields)) => fields
        .splitn(10, ' ')
        .last()
        .map(|path| json!({ "path": path, "conflicted": true })),
      Some(("? ", path)) => Some(json!({ "path": path, "untracked": true })),
      _ => None,
    };
    self.files.extend(file);
  }

  fn header(&mut self, header: &str) {
    let Some((name, value)) = header.split_once(' ') else {
      return;
    };
    match name {
      "branch.oid" if value != "(initial)" => self.head = Some(value.to_owned()),
      "branch.head" if value != "(detached)" => self.branch = Some(value.to_owned()),
      "branch.upstream" => self.upstream = Some(value.to_owned()),
      "branch.ab" => {
        let mut counts = value
          .split(' ')
          .map(|count| count.trim_start_matches(['+', '-']).parse().unwrap_or(0));
        self.ahead = counts.next().unwrap_or(0);
        self.behind = counts.next().unwrap_or(0);
      }
      _ => {}
    }
  }
}

/// A file's state on one side (`0` the index, `1` the working tree) of porcelain's `XY`; `None` for unchanged.
fn state(xy: &str, side: usize) -> Option<&'static str> {
  match xy.as_bytes().get(side) {
    Some(b'M') => Some("modified"),
    Some(b'T') => Some("type_changed"),
    Some(b'A') => Some("added"),
    Some(b'D') => Some("deleted"),
    Some(b'R') => Some("renamed"),
    Some(b'C') => Some("copied"),
    _ => None,
  }
}

/// The operation in progress in the repository: `merge`, `rebase`, `cherry_pick` or `revert`; `None` without one.
pub(super) fn in_progress(git: &Git) -> Result<Option<&'static str>, ToolError> {
  const MARKS: [(&str, &str); 5] = [
    ("MERGE_HEAD", "merge"),
    ("rebase-merge", "rebase"),
    ("rebase-apply", "rebase"),
    ("CHERRY_PICK_HEAD", "cherry_pick"),
    ("REVERT_HEAD", "revert"),
  ];
  let mut args = vec!["rev-parse"];
  for (mark, _) in MARKS {
    args.extend(["--git-path", mark]);
  }
  let paths = git.ok(&args)?;
  Ok(
    paths
      .lines()
      .zip(MARKS)
      .find(|(path, _)| git.dir.join(Path::new(path.trim())).exists())
      .map(|(_, (_, operation))| operation),
  )
}

/// `git_log`: commits of a revision or range, or of every branch, newest first, touching a path when it names one.
pub(super) fn log((programs, git): (&Programs, &Git), asked: &Log) -> Result<Value, ToolError> {
  let limit = asked.limit.unwrap_or(LOG_LIMIT).clamp(1, LOG_MAX).to_string();
  let mut args = vec![
    "log",
    "--format=%H%x1f%h%x1f%an%x1f%ae%x1f%aI%x1f%P%x1f%s%x1e",
    "-n",
    &limit,
  ];
  if asked.all {
    args.push("--all");
  } else if let Some(range) = &asked.range {
    args.extend(["--end-of-options", revision("range", range)?]);
  }
  if let Some(path) = &asked.path {
    args.extend(["--", names::path("path", path)?]);
  }
  let (text, truncated) = git.capped(programs, &args)?;
  let commits: Vec<Value> = text
    .split('\u{1e}')
    .map(str::trim)
    .filter(|record| !record.is_empty())
    .map(|record| {
      let fields: Vec<&str> = record.split('\u{1f}').collect();
      let field = |index: usize| fields.get(index).copied().unwrap_or_default();
      json!({
        "commit": field(0),
        "short": field(1),
        "author": field(2),
        "email": field(3),
        "date": field(4),
        "parents": field(5).split_whitespace().collect::<Vec<_>>(),
        "subject": field(6),
      })
    })
    .collect();
  Ok(json!({ "commits": commits, "truncated": truncated }))
}

/// `git_diff`: the working tree's, the index's or two revisions' differences, as a patch or as counts.
pub(super) fn diff((programs, git): (&Programs, &Git), asked: &Diff) -> Result<Value, ToolError> {
  if asked.to.is_some() && asked.from.is_none() {
    return Err(ToolError::InvalidArguments("to: needs from".to_owned()));
  }
  let mut args = vec!["diff", "--no-ext-diff"];
  if asked.staged {
    args.push("--cached");
  }
  if asked.stat {
    args.push("--stat");
  }
  args.push("--end-of-options");
  for (name, value) in [("from", &asked.from), ("to", &asked.to)] {
    if let Some(value) = value {
      args.push(revision(name, value)?);
    }
  }
  args.push("--");
  args.extend(names::paths("paths", &asked.paths)?);
  answered(git.capped(programs, &args)?)
}

/// `git_show`: a commit with its patch or counts, or a file's text at a revision.
pub(super) fn show((programs, git): (&Programs, &Git), asked: &Show) -> Result<Value, ToolError> {
  let rev = revision("rev", &asked.rev)?;
  let text = match &asked.file {
    Some(file) => {
      let at = format!("{rev}:{}", names::path("file", file)?);
      git.capped(programs, &["show", "--end-of-options", &at])?
    }
    None => {
      let mut args = vec!["show", "--no-ext-diff", "--format=fuller"];
      if asked.stat {
        args.push("--stat");
      }
      args.extend(["--end-of-options", rev]);
      git.capped(programs, &args)?
    }
  };
  answered(text)
}

/// `git_blame`: who last changed each line of a file, at a revision or in the working tree.
pub(super) fn blame((programs, git): (&Programs, &Git), asked: &Blame) -> Result<Value, ToolError> {
  let lines = match (asked.start, asked.end) {
    (Some(start), Some(end)) => Some(format!("{start},{end}")),
    (Some(start), None) => Some(format!("{start},")),
    (None, Some(end)) => Some(format!("1,{end}")),
    (None, None) => None,
  };
  let mut args = vec!["blame", "--date=short"];
  if let Some(lines) = &lines {
    args.extend(["-L", lines]);
  }
  if let Some(rev) = &asked.rev {
    args.extend(["--end-of-options", revision("rev", rev)?]);
  }
  args.extend(["--", names::path("path", &asked.path)?]);
  answered(git.capped(programs, &args)?)
}

/// `git_branches`: local and remote branches, each with its commit, upstream and how far ahead and behind it is.
pub(super) fn branches(git: &Git, _: &Nothing) -> Result<Value, ToolError> {
  let format = "--format=%(refname)%1f%(objectname:short)%1f%(upstream:short)%1f%(upstream:track,nobracket)%1f%(HEAD)\
                %1f%(committerdate:iso-strict)%1f%(contents:subject)";
  let text = git.ok(&["for-each-ref", format, "refs/heads", "refs/remotes"])?;
  let (mut local, mut remote) = (Vec::new(), Vec::new());
  for line in text.lines() {
    let fields: Vec<&str> = line.split('\u{1f}').collect();
    let field = |index: usize| fields.get(index).copied().unwrap_or_default();
    if let Some(name) = field(0).strip_prefix("refs/heads/") {
      let (ahead, behind, gone) = track(field(3));
      local.push(json!({
        "name": name,
        "commit": field(1),
        "current": field(4) == "*",
        "upstream": (!field(2).is_empty()).then(|| field(2)),
        "ahead": ahead,
        "behind": behind,
        "upstream_gone": gone,
        "date": field(5),
        "subject": field(6),
      }));
    } else if let Some(name) = field(0).strip_prefix("refs/remotes/")
      && !name.ends_with("/HEAD")
    {
      remote.push(json!({ "name": name, "commit": field(1), "date": field(5), "subject": field(6) }));
    }
  }
  Ok(json!({ "local": local, "remote": remote }))
}

/// How far a branch is from its upstream, from `%(upstream:track,nobracket)`: ahead, behind, and whether it is gone.
fn track(text: &str) -> (u32, u32, bool) {
  let mut counts = (0, 0, text == "gone");
  for part in text.split(", ") {
    if let Some(count) = part.strip_prefix("ahead ") {
      counts.0 = count.parse().unwrap_or(0);
    } else if let Some(count) = part.strip_prefix("behind ") {
      counts.1 = count.parse().unwrap_or(0);
    }
  }
  counts
}

/// `git_remotes`: each remote with where it fetches from and pushes to, credentials removed.
pub(super) fn remotes(git: &Git, _: &Nothing) -> Result<Value, ToolError> {
  let text = git.ok(&["remote", "-v"])?;
  let mut remotes: Vec<Value> = Vec::new();
  for line in text.lines() {
    let Some((name, rest)) = line.split_once('\t') else {
      continue;
    };
    let Some((url, way)) = rest.rsplit_once(' ') else {
      continue;
    };
    let url = redact_userinfo(url);
    let key = if way == "(push)" { "push" } else { "fetch" };
    match remotes.iter_mut().find(|remote| remote["name"] == name) {
      Some(remote) => remote[key] = Value::String(url),
      None => {
        let mut remote = json!({ "name": name });
        remote[key] = Value::String(url);
        remotes.push(remote);
      }
    }
  }
  Ok(json!({ "remotes": remotes }))
}

/// git's text as a read answers it, and whether it was cut.
fn answered((text, truncated): (String, bool)) -> Result<Value, ToolError> {
  Ok(json!({ "text": text, "truncated": truncated }))
}
