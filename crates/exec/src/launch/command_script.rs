//! Windows `.cmd`/`.bat` scripts: recognising npm's Node.js launchers and the arguments that
//! pass through `cmd.exe` unchanged.

use std::{
  fs, io,
  path::{Path, PathBuf},
};

use super::{Launch, Via};

/// Node.js next to an npm launcher; the launcher falls back to the one on the `PATH`.
const LOCAL_NODE: &str = "node.exe";
const PATH_NODE: &str = "node";
/// The launchers' own folder, as `cmd-shim` (`%dp0%`) and corepack (`%~dp0`) write it.
const FOLDER_PREFIXES: [&str; 2] = ["%dp0%\\", "%~dp0\\"];
/// How the launchers name Node.js before the script: `cmd-shim` picks `%_prog%` (Node.js next
/// to it, else on the `PATH`), corepack names it directly.
const NODE_PROGRAMS: [&str; 4] = ["\"%_prog%\"", "\"%dp0%\\node.exe\"", "\"%~dp0\\node.exe\"", "node"];
/// Confirms that `%_prog%` of a `cmd-shim` launcher is Node.js.
const CMD_SHIM_NODE: &str = "SET \"_prog=%dp0%\\node.exe\"";
const SCRIPT_EXTENSIONS: [&str; 3] = ["js", "cjs", "mjs"];

/// `None` when `program` is not a command script, or is missing (starting it reports that).
pub(super) fn resolve(program: &Path) -> Option<Launch> {
  let extension = program.extension()?.to_str()?.to_ascii_lowercase();
  if extension != "cmd" && extension != "bat" {
    return None;
  }
  let text = match fs::read_to_string(program) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
    Err(error) => {
      tracing::warn!(%error, program = %program.display(), "cannot read the command script; it runs through cmd.exe");
      return Some(command_script(program));
    }
  };
  let folder = program.parent()?;
  let script = node_shim_script(&text)
    .map(|relative| folder.join(relative))
    .filter(|script| script.is_file());
  let Some(script) = script else {
    return Some(command_script(program));
  };
  let local = folder.join(LOCAL_NODE);
  let node = if local.is_file() {
    local
  } else {
    PathBuf::from(PATH_NODE)
  };
  Some(Launch {
    program: node,
    leading_args: vec![script.to_string_lossy().into_owned()],
    via: Via::NodeScript { script },
  })
}

fn command_script(program: &Path) -> Launch {
  Launch {
    program: program.to_path_buf(),
    leading_args: Vec::new(),
    via: Via::CommandScript,
  }
}

/// The script, relative to the launcher's folder, that an npm launcher (`cmd-shim` or corepack)
/// runs with Node.js, passing its own arguments on. `None` for any other script, or one whose
/// lines name different scripts.
pub fn node_shim_script(text: &str) -> Option<PathBuf> {
  let mut found: Option<&str> = None;
  for line in text.lines() {
    let Some((program, script)) = launch_line(line) else {
      continue;
    };
    if program == NODE_PROGRAMS[0] && !text.contains(CMD_SHIM_NODE) {
      return None;
    }
    match found {
      Some(earlier) if earlier != script => return None,
      _ => found = Some(script),
    }
  }
  found.map(|script| script.split('\\').collect())
}

/// `<node> "<folder>\<script>" %*`: the Node.js program as written and the script's relative path.
fn launch_line(line: &str) -> Option<(&'static str, &str)> {
  let rest = line.trim_end().strip_suffix("%*")?.trim_end().strip_suffix('"')?;
  let (before, quoted) = rest.rsplit_once('"')?;
  let script = FOLDER_PREFIXES.iter().find_map(|prefix| quoted.strip_prefix(prefix))?;
  let is_script = Path::new(script)
    .extension()
    .and_then(|extension| extension.to_str())
    .is_some_and(|extension| SCRIPT_EXTENSIONS.contains(&extension));
  if !is_script || script.contains('%') {
    return None;
  }
  let before = before.trim_end();
  let program = NODE_PROGRAMS.into_iter().find(|program| {
    before
      .strip_suffix(program)
      .is_some_and(|start| start.is_empty() || start.ends_with([' ', '\t', '&', ')']))
  })?;
  Some((program, script))
}

/// Whether `cmd.exe` hands `arg` to a script's program unchanged: letters, digits, spaces (Rust
/// quotes the argument) and `-_.,/:=@+`. Quotes, `%`, `!`, `^`, `&`, `|`, `<`, `>`, brackets,
/// backslashes and line breaks are read by `cmd.exe` or change the program's own parsing.
pub fn passes_through_cmd(arg: &str) -> bool {
  !arg.is_empty()
    && arg
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || " -_.,/:=@+".contains(c))
}
