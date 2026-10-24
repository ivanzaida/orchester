//! What the chat's agent reads of each git tool.

/// What every write says of the person's switch and of the run worktrees.
const WRITE: &str = "Only while the person turned on Let the Orchestrator write with git in Settings; off, it is \
  refused (write_off) and you tell the person. Never in a turn Orchester started by itself (person_only). It works in \
  the project's checkout, or with run in that run's worktree once the run ended (run_active while it has not). The \
  conversation shows what ran and what git said; tell the person what you did.";

pub(super) const STATUS: &str = "Reads the git state of the project's checkout (or with run, that run's worktree): \
  the branch, its upstream and how far ahead and behind, a merge or rebase in progress, and every changed, staged, \
  conflicted and untracked file. Changes nothing.";

pub(super) const LOG: &str = "Reads commits, newest first: of range (a revision such as main, or a range such as \
  main..feature), or of every branch with all, only those touching path when given, limit at most 200 (30 by \
  default). Changes nothing.";

pub(super) const DIFF: &str = "Reads differences as a patch, or with stat as files and counts: the working tree's \
  unstaged changes; with staged, the staged ones; with from (and to), between revisions or a revision and the working \
  tree; only under paths when given. Long output is cut (truncated). Changes nothing.";

pub(super) const SHOW: &str = "Reads a commit (rev) with its patch, or with stat its files; or with file, that file's \
  text as rev holds it. Long output is cut (truncated). Changes nothing.";

pub(super) const BRANCHES: &str = "Reads local branches with their commit, upstream and how far ahead and behind it \
  each is (upstream_gone when the upstream was deleted), and the remote-tracking branches. Changes nothing; call \
  git_fetch first for the remotes' latest.";

pub(super) const BLAME: &str = "Reads who last changed each line of path (lines start to end), at rev or in the \
  working tree. Changes nothing.";

pub(super) const REMOTES: &str = "Reads the remotes, with the addresses they fetch from and push to (credentials \
  removed). Changes nothing.";

pub(super) fn commit() -> String {
  format!(
    "Commits with message: exactly paths (new files among them) when given, every change with all, else what is \
     staged; the author is git's own setting (author_unknown when git has none). Refused when there is nothing to \
     commit (nothing_to_commit). {WRITE}"
  )
}

pub(super) fn create_branch() -> String {
  format!("Creates branch name at start (HEAD by default), and switches to it with switch. {WRITE}")
}

pub(super) fn switch() -> String {
  format!(
    "Switches to branch only as git does, keeping every uncommitted change; when git would overwrite one it is \
     refused (would_lose_work): suggest committing first, or propose switch_discarding with git_propose if the \
     person wants the changes gone. {WRITE}"
  )
}

pub(super) fn pull() -> String {
  format!(
    "Pulls the branch's upstream (or remote and branch), fast-forward only; when the branches diverged it is refused \
     in git's words. With merge, only when the person asked, it merges; a conflict is undone and refused \
     (conflict, naming the files) unless keep_conflicts, only when the person asked to resolve it themselves. A \
     remote that wants credentials is refused at once (auth_failed): git never asks for them here; the person signs \
     in to git in a terminal. A pull that takes too long is stopped (timed_out). {WRITE}"
  )
}

pub(super) fn push() -> String {
  format!(
    "Pushes branch (the current one by default) to the remote's branch of the same name (the upstream's remote, \
     else origin), with set_upstream making it the upstream. Never forced: a rejected push is told in git's words \
     (git_failed); pull first, or propose force_push with git_propose only when the person asks. Credentials as for \
     git_pull (auth_failed); a slow remote is stopped (timed_out). {WRITE}"
  )
}

pub(super) fn merge() -> String {
  format!(
    "Merges branch into the current branch (no_ff for a merge commit always, message for its message). A conflict \
     is undone and refused (conflict, naming the files) unless keep_conflicts, only when the person asked to resolve \
     it themselves; git_propose abort_merge then proposes undoing it. Local changes the merge would overwrite are \
     refused (would_lose_work). {WRITE}"
  )
}

pub(super) fn revert() -> String {
  format!(
    "Makes a new commit that undoes commit (mainline for a merge commit's parent). A conflict is undone and refused \
     (conflict). {WRITE}"
  )
}

pub(super) fn tag() -> String {
  format!("Tags rev (HEAD by default) as name; with message, an annotated tag. {WRITE}")
}

pub(super) fn fetch() -> String {
  format!(
    "Fetches the upstream's remote (or remote, else origin; every remote with all); prune removes remote-tracking \
     branches the remote no longer has, never a local branch. Credentials and slowness as for git_pull. {WRITE}"
  )
}

pub(super) const PROPOSE: &str = "Proposes a git operation that can lose work, as a card the person applies or \
  dismisses in the conversation, whatever their switches say; you never apply it, and nothing changes until they do. \
  operation.kind is one of: reset (to, mode hard|mixed|soft: the branch's commits after to leave it, hard also \
  discards uncommitted changes), clean (paths, directories, ignored: deletes untracked files), delete_branch (branch, \
  remote for the remote's branch), force_push (remote, branch: over what the remote holds, leased to what the card \
  shows), discard_changes (paths: uncommitted changes to tracked files), switch_discarding (branch), rebase (onto; a \
  conflict undoes it), delete_tag (tag, remote), abort_merge. The card shows the commands and what would be lost \
  (commits no other branch holds, changes, files); applying it is refused when any of that changed since \
  (changed_since_proposed). Propose one only when the person asked for that outcome; there is no stash: suggest a \
  commit on a branch instead. With run, in that run's worktree once the run ended (run_active while it has not).";
