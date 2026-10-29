//! The inline script a shell runs (`bash -c …`, `pwsh -Command …`), read only when the shell is started
//! with options that change nothing it reads at start-up.

use super::words::Quoting;

/// For a shell, the script it runs and its quoting, or `Some(None)` when it runs one Orchester cannot
/// read (a script file, an encoded command) or is started with an option that is not listed (a startup
/// file, another working folder); `None` for any other program.
pub(super) fn shell_script(program: &str, args: &[String]) -> Option<Option<(String, Quoting)>> {
  match program {
    "bash" | "sh" | "zsh" => Some(posix_script(args)),
    "pwsh" | "powershell" => Some(powershell_script(args)),
    _ => None,
  }
}

/// `bash -c script` with listed options before it: clusters of option letters that change no file
/// read at start-up, `-o name`, `--login`, `--noprofile`, `--norc`.
fn posix_script(args: &[String]) -> Option<(String, Quoting)> {
  const LETTERS: &str = "abefhklmnptuvxBCHP";
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--login" | "--noprofile" | "--norc" => {}
      "-o" | "+o" => {
        args.next()?;
      }
      cluster if cluster.starts_with('-') && cluster.len() > 1 => {
        let letters = &cluster[1..];
        if !letters.chars().all(|letter| letter == 'c' || LETTERS.contains(letter)) {
          return None;
        }
        if letters.contains('c') {
          return args.next().map(|script| (script.clone(), Quoting::Bash));
        }
      }
      _ => return None,
    }
  }
  None
}

/// `pwsh -Command script` with listed parameters before it (matched without case).
fn powershell_script(args: &[String]) -> Option<(String, Quoting)> {
  const SWITCHES: [&str; 10] = [
    "-noprofile",
    "-nop",
    "-noninteractive",
    "-noni",
    "-nologo",
    "-nol",
    "-mta",
    "-sta",
    "-noexit",
    "-noprofileloadtime",
  ];
  const VALUED: [&str; 4] = ["-executionpolicy", "-ep", "-outputformat", "-of"];
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    let lower = arg.to_lowercase();
    if lower.len() >= 2 && "-command".starts_with(&lower) {
      let script = args.map(String::as_str).collect::<Vec<_>>().join(" ");
      return (!script.is_empty()).then_some((script, Quoting::PowerShell));
    }
    if VALUED.contains(&lower.as_str()) {
      args.next()?;
    } else if !SWITCHES.contains(&lower.as_str()) {
      return None;
    }
  }
  None
}
