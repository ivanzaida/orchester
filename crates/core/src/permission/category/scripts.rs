//! `sed` and `awk`, which run a script of their own over the files they read. Their script is never a
//! path. `sed` prints unless `-i` edits its files in place; a script is read when its commands only
//! print, select, change or add lines ([`sed_script::harmless`]), or when it holds none of the letters
//! of the commands that write a file (`w`, `W`), read one (`r`, `R`) or run a command (`e`). `awk` is
//! read only with a program that uses the names [`awk_program`] allows, the options that change
//! neither what it reads nor what it runs, and command-line assignments of the variables it may set.

use super::{
  awk_program,
  options::{self, Known, Takes},
  programs::{Judged, Kind, operand_positions},
  sed_script,
};

/// The verdict for `sed` and `awk`; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  Some(match program {
    "sed" => sed(args),
    "awk" | "gawk" | "mawk" | "nawk" => awk(args),
    _ => return None,
  })
}

/// `sed`'s long options that change neither what it reads nor what it writes, besides `--in-place`.
const SED_LONG: [&str; 10] = [
  "quiet",
  "silent",
  "regexp-extended",
  "posix",
  "debug",
  "sandbox",
  "null-data",
  "zero-terminated",
  "separate",
  "unbuffered",
];

fn sed(args: &[String]) -> Judged {
  let Some(in_place) = sed_in_place(args) else {
    return Judged::unknown();
  };
  let positions = operand_positions(args);
  let Some((&at, files)) = positions.split_first() else {
    return Judged::unknown();
  };
  let script = args[at].as_str();
  if !sed_script::harmless(script) && script.contains(['w', 'W', 'e', 'r', 'R']) {
    return Judged::unknown();
  }
  if in_place {
    return Judged::writing(Kind::Writes, files.iter().map(|&file| args[file].clone()).collect()).with_script(Some(at));
  }
  Judged::of(Kind::ReadOnly).known_safe().with_script(Some(at))
}

/// Whether `sed`'s options edit its files in place (`-i`, `-i.bak`, `-ni`, `--in-place`); `None` for
/// any option not listed, such as a script given as one (`-e`, `-f`) or an abbreviated long option,
/// which `sed` would take for the option it begins.
fn sed_in_place(args: &[String]) -> Option<bool> {
  let mut in_place = false;
  for arg in args.iter().take_while(|arg| *arg != "--") {
    if let Some(long) = arg.strip_prefix("--") {
      let name = long.split_once('=').map_or(long, |(name, _)| name);
      if name == "in-place" {
        in_place = true;
      } else if !SED_LONG.contains(&name) {
        return None;
      }
    } else if let Some(cluster) = arg.strip_prefix('-').filter(|cluster| !cluster.is_empty()) {
      for letter in cluster.chars() {
        // What follows `-i` is the backup suffix.
        if letter == 'i' {
          in_place = true;
          break;
        }
        if !"nErsuz".contains(letter) {
          return None;
        }
      }
    }
  }
  Some(in_place)
}

/// `awk` options that change neither what it reads nor what it runs.
const AWK: [Known; 4] = [
  ("-F", Takes::Value),
  ("-v", Takes::Value),
  ("--field-separator", Takes::Value),
  ("--assign", Takes::Value),
];

fn awk(args: &[String]) -> Judged {
  let Some(parsed) = options::parse(args, &AWK) else {
    return Judged::unknown();
  };
  let Some((program, files)) = parsed.operands.split_first() else {
    return Judged::unknown();
  };
  // `-v name=value` and an operand `name=value` set a variable before the records are read.
  let assigned = parsed.values(&["-v", "--assign"]).chain(
    files
      .iter()
      .map(String::as_str)
      .filter(|file| assignment(file).is_some()),
  );
  let settable = assigned
    .into_iter()
    .all(|assignment| self::assignment(assignment).is_some_and(awk_program::settable));
  if !(settable && awk_program::harmless(program)) {
    return Judged::unknown();
  }
  let at = args.iter().position(|arg| arg == program);
  Judged::of(Kind::ReadOnly).known_safe().with_script(at)
}

/// The variable `word` sets, when it is an assignment (`name=value`).
fn assignment(word: &str) -> Option<&str> {
  let (name, _) = word.split_once('=')?;
  let plain = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
  plain.then_some(name)
}
