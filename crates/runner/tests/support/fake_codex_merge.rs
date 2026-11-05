//! What a turn of `orchester-fake-codex` does with the conflicts of a moved base the runner merged into its worktree:
//! as a sandboxed Codex agent can, it only edits the files the notice names, never runs git. It keeps both sides of
//! each conflict, dropping git's marker lines, unless `fake-leave-conflicts` lies next to the worktrees (a test that
//! has an agent end its step with the markers left).

use std::path::Path;

/// The notice's lead to the files git stopped at conflicts in, one per line as "- `<path>`".
const CONFLICTS: &str = "Git stopped at conflicts in these files";

/// Resolves the conflicts `prompt` hands the agent working in `worktree`, if it hands any.
pub fn resolve(worktree: &Path, prompt: &str) {
  let Some((_, listed)) = prompt.split_once(CONFLICTS) else {
    return;
  };
  let left = worktree
    .parent()
    .and_then(Path::parent)
    .is_some_and(|orchester| orchester.join("fake-leave-conflicts").exists());
  if left {
    return;
  }
  let files = listed
    .lines()
    .skip(1)
    .map_while(|line| line.strip_prefix("- `")?.strip_suffix('`'));
  for file in files {
    let path = worktree.join(file);
    let resolved = std::fs::read_to_string(&path).and_then(|text| {
      let kept: String = text
        .lines()
        .filter(|line| {
          !["<<<<<<<", "=======", ">>>>>>>"]
            .iter()
            .any(|marker| line.starts_with(marker))
        })
        .map(|line| format!("{line}\n"))
        .collect();
      std::fs::write(&path, kept)
    });
    if let Err(error) = resolved {
      eprintln!("cannot resolve the conflicts in {file}: {error}");
    }
  }
}
