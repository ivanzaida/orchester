//! The arguments of the chat's git tools, as their input schemas tell the agent. Every tool works in the chat's own
//! project (a `project` the call names is checked and taken off first) and takes, optionally, the `run` whose
//! worktree it works in (taken off too, and added to each schema by [`with_run`]).

use rmcp::{model::JsonObject, schemars::JsonSchema};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::app_mcp::ToolError;

/// `arguments` read as the tool's `T`.
pub(super) fn parse<T: DeserializeOwned>(arguments: &JsonObject) -> Result<T, ToolError> {
  serde_json::from_value(Value::Object(arguments.clone()))
    .map_err(|error| ToolError::InvalidArguments(error.to_string()))
}

/// A tool that takes nothing but where it works.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Nothing {}

/// What every tool's schema says of `run`, which [`super::call`] takes off the arguments before they are read.
pub(super) const RUN: &str = "A run's id or number (#12): its worktree instead of the project's checkout. Writing   there is refused while the run has not ended.";

/// `schema` with the `run` every tool takes.
pub(super) fn with_run(mut schema: JsonObject) -> JsonObject {
  if let Some(Value::Object(properties)) = schema.get_mut("properties") {
    properties.insert("run".to_owned(), json!({ "type": "string", "description": RUN }));
  } else {
    schema.insert(
      "properties".to_owned(),
      json!({ "run": { "type": "string", "description": RUN } }),
    );
  }
  schema
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Log {
  /// A revision or range (`main`, `main..feature`, `v1.2..HEAD`); `HEAD` when left out.
  #[serde(default)]
  pub(super) range: Option<String>,
  /// Only commits that changed this path.
  #[serde(default)]
  pub(super) path: Option<String>,
  /// At most this many commits, 30 when left out, at most 200.
  #[serde(default)]
  pub(super) limit: Option<u32>,
  /// Every branch's commits, local and remote, instead of `range`.
  #[serde(default)]
  pub(super) all: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Diff {
  /// The revision compared from; left out, the working tree's changes against the index (or with `staged`, the index
  /// against HEAD).
  #[serde(default)]
  pub(super) from: Option<String>,
  /// The revision compared to; left out, the working tree.
  #[serde(default)]
  pub(super) to: Option<String>,
  /// Only these paths.
  #[serde(default)]
  pub(super) paths: Vec<String>,
  /// The staged changes (`--cached`).
  #[serde(default)]
  pub(super) staged: bool,
  /// Only the files and their counts (`--stat`).
  #[serde(default)]
  pub(super) stat: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Show {
  /// A commit, tag or other revision.
  pub(super) rev: String,
  /// The text of this file as `rev` holds it, instead of the commit.
  #[serde(default)]
  pub(super) file: Option<String>,
  /// Only the commit's files and their counts.
  #[serde(default)]
  pub(super) stat: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Blame {
  /// The file.
  pub(super) path: String,
  /// The revision blamed; the working tree when left out.
  #[serde(default)]
  pub(super) rev: Option<String>,
  /// The first line, from 1.
  #[serde(default)]
  pub(super) start: Option<u32>,
  /// The last line.
  #[serde(default)]
  pub(super) end: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Commit {
  /// The commit's message.
  pub(super) message: String,
  /// Commit exactly these paths as they are in the working tree, new files among them; other staged changes stay
  /// staged.
  #[serde(default)]
  pub(super) paths: Vec<String>,
  /// Commit every change, new files among them (`git add -A`). Without `paths` or `all`, what is staged.
  #[serde(default)]
  pub(super) all: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct CreateBranch {
  /// The new branch's name.
  pub(super) name: String,
  /// Where it starts; HEAD when left out.
  #[serde(default)]
  pub(super) start: Option<String>,
  /// Switch to it too.
  #[serde(default)]
  pub(super) switch: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Switch {
  /// The branch; a remote's branch of that name is checked out as a new local branch that tracks it.
  pub(super) branch: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Pull {
  /// A remote the repository lists (git_remotes); the branch's upstream when left out.
  #[serde(default)]
  pub(super) remote: Option<String>,
  /// The remote's branch, by its plain name, with `remote`.
  #[serde(default)]
  pub(super) branch: Option<String>,
  /// Merge when the branches diverged, instead of only fast-forwarding. Only when the person asked for it.
  #[serde(default)]
  pub(super) merge: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Push {
  /// A remote the repository lists (git_remotes); the branch's upstream remote, else `origin`, when left out.
  #[serde(default)]
  pub(super) remote: Option<String>,
  /// The local branch pushed to the remote's branch of the same name; the current branch when left out.
  #[serde(default)]
  pub(super) branch: Option<String>,
  /// Make the remote's branch the branch's upstream (`-u`).
  #[serde(default)]
  pub(super) set_upstream: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Merge {
  /// The branch merged into the current branch, by its name (`feature`, `origin/main`).
  pub(super) branch: String,
  /// Always make a merge commit (`--no-ff`).
  #[serde(default)]
  pub(super) no_ff: bool,
  /// The merge commit's message; git's own when left out.
  #[serde(default)]
  pub(super) message: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Revert {
  /// The commit undone by a new commit.
  pub(super) commit: String,
  /// For a merge commit: the parent, from 1, whose side is kept (`-m`).
  #[serde(default)]
  pub(super) mainline: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Tag {
  /// The tag's name.
  pub(super) name: String,
  /// What it tags; HEAD when left out.
  #[serde(default)]
  pub(super) rev: Option<String>,
  /// With a message, an annotated tag.
  #[serde(default)]
  pub(super) message: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Fetch {
  /// A remote the repository lists (git_remotes); the branch's upstream remote, else `origin`, when left out.
  #[serde(default)]
  pub(super) remote: Option<String>,
  /// Every remote.
  #[serde(default)]
  pub(super) all: bool,
  /// Remove remote-tracking branches the remote no longer has (`--prune`); no local branch is touched.
  #[serde(default)]
  pub(super) prune: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) struct Propose {
  pub(super) operation: Operation,
}

/// An operation that can lose work.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub(super) enum Operation {
  /// Move the current branch to `to` (`git reset`): `hard` also discards uncommitted changes to tracked files,
  /// `mixed` unstages them, `soft` keeps them staged.
  Reset { to: String, mode: Mode },
  /// Delete untracked files (`git clean -f`), only under `paths` when given; `directories` deletes untracked
  /// directories too, `ignored` ignored files too.
  Clean {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    directories: bool,
    #[serde(default)]
    ignored: bool,
  },
  /// Delete a local branch (`git branch -D`), or with `remote`, the remote's branch (`git push --delete`).
  DeleteBranch {
    branch: String,
    #[serde(default)]
    remote: Option<String>,
  },
  /// Push a branch over what the remote holds (`--force-with-lease`): the current branch and its upstream remote, else
  /// `origin`, when left out.
  ForcePush {
    #[serde(default)]
    remote: Option<String>,
    #[serde(default)]
    branch: Option<String>,
  },
  /// Discard the uncommitted changes to tracked files under `paths`, staged and not (`git restore`).
  DiscardChanges { paths: Vec<String> },
  /// Switch to a branch, discarding uncommitted changes to tracked files (`git switch --discard-changes`).
  SwitchDiscarding { branch: String },
  /// Rebase the current branch onto `onto` (`git rebase`); a conflict undoes it.
  Rebase { onto: String },
  /// Delete a tag (`git tag -d`), or with `remote`, the remote's tag.
  DeleteTag {
    tag: String,
    #[serde(default)]
    remote: Option<String>,
  },
  /// Abort the merge in progress (`git merge --abort`).
  AbortMerge,
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub(super) enum Mode {
  Hard,
  Mixed,
  Soft,
}
