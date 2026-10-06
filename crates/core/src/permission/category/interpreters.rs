//! Script files run by an interpreter (`python .tmp/verify/start.py`, `pwsh -File build.ps1`): in a step
//! that edits files, a script in the worktree runs nothing the agent could not run by writing a test and
//! running an allowed `cargo test` (the user's decision of 2026-10-06), so such a part counts as an edit
//! of the worktree when the script is a regular file there ([`super::place::Placing::plain_file`]). Only
//! the forms that run the file named are read: no inline code (`-c`, `-e`, `-Command`,
//! `-EncodedCommand`), no module (`-m`), no option that loads code from elsewhere (`node -r`, `bash
//! --rcfile`). The script's own arguments are placed like any other.

/// Python options that change neither what runs nor where code is loaded from.
const PYTHON_FLAGS: [&str; 7] = ["-u", "-B", "-E", "-s", "-I", "-O", "-OO"];

/// Python's `-X` options that only report or check (`-X importtime`); others write elsewhere
/// (`pycache_prefix`) or load code (`presite`).
const PYTHON_X: [&str; 5] = ["importtime", "utf8", "dev", "faulthandler", "tracemalloc"];

/// Shell options that change neither what runs nor where code is loaded from (`set -e`, `-u`, `-x`,
/// `-v`).
const SHELL_LETTERS: &str = "euxv";

/// PowerShell switches that change neither what runs nor where code is loaded from; `-NoProfile` is
/// required, since a profile is the user's code.
const POWERSHELL_SWITCHES: [&str; 6] = ["-noprofile", "-nop", "-noninteractive", "-noni", "-nologo", "-nol"];

/// Where among `args` the script file `program` runs is; `None` when it runs no file or one Orchester does
/// not read.
pub(super) fn script(program: &str, args: &[String]) -> Option<usize> {
  match program {
    "python" | "python3" | "py" => python(args),
    "node" => first_plain(args, |_| false),
    // zsh reads the user's `.zshenv` whatever its options.
    "bash" | "sh" => first_plain(args, |arg| {
      arg.len() > 1 && arg[1..].chars().all(|letter| SHELL_LETTERS.contains(letter))
    }),
    "pwsh" | "powershell" => powershell(args),
    _ => None,
  }
}

/// `python [flags] script …`; `-X name` (attached or not, one of [`PYTHON_X`]) sets an implementation
/// option, `py -3` or `py -3.12` chooses the version.
fn python(args: &[String]) -> Option<usize> {
  let mut at = 0;
  while let Some(arg) = args.get(at) {
    if !arg.starts_with('-') {
      return Some(at);
    }
    at += match arg.as_str() {
      "-X" if args.get(at + 1).is_some_and(|value| PYTHON_X.contains(&value.as_str())) => 2,
      flag if PYTHON_FLAGS.contains(&flag) => 1,
      flag if flag.strip_prefix("-X").is_some_and(|value| PYTHON_X.contains(&value)) => 1,
      version if version.len() > 1 && version[1..].chars().all(|c| c.is_ascii_digit() || c == '.') => 1,
      _ => return None,
    };
  }
  None
}

/// The first argument that is no option, every option before it one that `allowed` takes.
fn first_plain(args: &[String], allowed: impl Fn(&str) -> bool) -> Option<usize> {
  let at = args.iter().position(|arg| !arg.starts_with('-'))?;
  args[..at].iter().all(|arg| allowed(arg)).then_some(at)
}

/// `pwsh -NoProfile [switches] [-ExecutionPolicy Bypass|RemoteSigned] -File script …`, names matched
/// without case.
fn powershell(args: &[String]) -> Option<usize> {
  let mut at = 0;
  let mut no_profile = false;
  while let Some(arg) = args.get(at) {
    match arg.to_lowercase().as_str() {
      "-file" | "-f" => {
        return args
          .get(at + 1)
          .is_some_and(|script| no_profile && !script.starts_with('-'))
          .then_some(at + 1);
      }
      "-noprofile" | "-nop" => {
        no_profile = true;
        at += 1;
      }
      "-executionpolicy" | "-ep" => {
        let policy = args.get(at + 1)?.to_lowercase();
        if !matches!(policy.as_str(), "bypass" | "remotesigned") {
          return None;
        }
        at += 2;
      }
      switch if POWERSHELL_SWITCHES.contains(&switch) => at += 1,
      _ => return None,
    }
  }
  None
}
