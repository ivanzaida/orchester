//! Programs that create, copy, move, link or delete the files they name, `find` and `sed`. `cp`, `ln`,
//! `install` and `mv` are read against the options they are known to take ([`options`]), so a target
//! written as an option (`--target-directory=…`) is never missed; a link's target must lie inside, as a
//! link to a folder outside would let later commands reach it through the worktree.

use super::{
  options::{self, Known, Takes},
  programs::{Judged, Kind, has_option, operands},
};
use crate::permission::known_safe;

/// Programs that write each file they name: every argument that is not an option.
const WRITES_ARGS: [&str; 6] = ["mkdir", "touch", "chmod", "chown", "tee", "truncate"];

/// Programs that remove each file they name.
const DELETES_ARGS: [&str; 3] = ["rm", "rmdir", "unlink"];

const CP: [Known; 34] = [
  ("-a", Takes::Nothing),
  ("-b", Takes::Nothing),
  ("-d", Takes::Nothing),
  ("-f", Takes::Nothing),
  ("-H", Takes::Nothing),
  ("-i", Takes::Nothing),
  ("-L", Takes::Nothing),
  ("-n", Takes::Nothing),
  ("-P", Takes::Nothing),
  ("-p", Takes::Nothing),
  ("-R", Takes::Nothing),
  ("-r", Takes::Nothing),
  ("-T", Takes::Nothing),
  ("-u", Takes::Nothing),
  ("-v", Takes::Nothing),
  ("-x", Takes::Nothing),
  ("-S", Takes::Value),
  ("-t", Takes::Value),
  ("--archive", Takes::Nothing),
  ("--backup", Takes::Nothing),
  ("--dereference", Takes::Nothing),
  ("--force", Takes::Nothing),
  ("--interactive", Takes::Nothing),
  ("--no-clobber", Takes::Nothing),
  ("--no-dereference", Takes::Nothing),
  ("--parents", Takes::Nothing),
  ("--preserve", Takes::Nothing),
  ("--recursive", Takes::Nothing),
  ("--no-target-directory", Takes::Nothing),
  ("--update", Takes::Nothing),
  ("--verbose", Takes::Nothing),
  ("--one-file-system", Takes::Nothing),
  ("--suffix", Takes::Value),
  ("--target-directory", Takes::Value),
];

const LN: [Known; 24] = [
  ("-b", Takes::Nothing),
  ("-d", Takes::Nothing),
  ("-F", Takes::Nothing),
  ("-f", Takes::Nothing),
  ("-i", Takes::Nothing),
  ("-L", Takes::Nothing),
  ("-n", Takes::Nothing),
  ("-P", Takes::Nothing),
  ("-r", Takes::Nothing),
  ("-s", Takes::Nothing),
  ("-T", Takes::Nothing),
  ("-v", Takes::Nothing),
  ("-S", Takes::Value),
  ("-t", Takes::Value),
  ("--force", Takes::Nothing),
  ("--interactive", Takes::Nothing),
  ("--logical", Takes::Nothing),
  ("--no-dereference", Takes::Nothing),
  ("--physical", Takes::Nothing),
  ("--relative", Takes::Nothing),
  ("--symbolic", Takes::Nothing),
  ("--verbose", Takes::Nothing),
  ("--suffix", Takes::Value),
  ("--target-directory", Takes::Value),
];

/// `install` without `--strip-program`, which starts a program.
const INSTALL: [Known; 21] = [
  ("-b", Takes::Nothing),
  ("-c", Takes::Nothing),
  ("-C", Takes::Nothing),
  ("-d", Takes::Nothing),
  ("-D", Takes::Nothing),
  ("-p", Takes::Nothing),
  ("-v", Takes::Nothing),
  ("-g", Takes::Value),
  ("-m", Takes::Value),
  ("-o", Takes::Value),
  ("-S", Takes::Value),
  ("-t", Takes::Value),
  ("--compare", Takes::Nothing),
  ("--directory", Takes::Nothing),
  ("--preserve-timestamps", Takes::Nothing),
  ("--verbose", Takes::Nothing),
  ("--group", Takes::Value),
  ("--mode", Takes::Value),
  ("--owner", Takes::Value),
  ("--suffix", Takes::Value),
  ("--target-directory", Takes::Value),
];

const MV: [Known; 18] = [
  ("-b", Takes::Nothing),
  ("-f", Takes::Nothing),
  ("-i", Takes::Nothing),
  ("-n", Takes::Nothing),
  ("-T", Takes::Nothing),
  ("-u", Takes::Nothing),
  ("-v", Takes::Nothing),
  ("-S", Takes::Value),
  ("-t", Takes::Value),
  ("--backup", Takes::Nothing),
  ("--force", Takes::Nothing),
  ("--interactive", Takes::Nothing),
  ("--no-clobber", Takes::Nothing),
  ("--no-target-directory", Takes::Nothing),
  ("--update", Takes::Nothing),
  ("--verbose", Takes::Nothing),
  ("--suffix", Takes::Value),
  ("--target-directory", Takes::Value),
];

const TARGET: [&str; 2] = ["-t", "--target-directory"];

/// The verdict for a file program; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  Some(match program {
    "cp" => copy(args, &CP),
    "install" if options::parse(args, &INSTALL).is_some_and(|parsed| parsed.has(&["-d", "--directory"])) => {
      Judged::writing(Kind::Writes, operands(args))
    }
    "install" => copy(args, &INSTALL),
    "ln" => link(args),
    "mv" => options::parse(args, &MV).map_or_else(Judged::unknown, |parsed| {
      let mut written = parsed.operands.clone();
      written.extend(parsed.values(&TARGET).map(str::to_owned));
      Judged::writing(Kind::Writes, written).deleting()
    }),
    "find" => find(args),
    "sed" => sed(args),
    program if WRITES_ARGS.contains(&program) => Judged::writing(Kind::Writes, operands(args)),
    program if DELETES_ARGS.contains(&program) => Judged::writing(Kind::Writes, operands(args)).deleting(),
    _ => return None,
  })
}

/// A copy writes `-t <folder>`, else its last operand, and reads the others.
fn copy(args: &[String], table: &[Known]) -> Judged {
  let Some(parsed) = options::parse(args, table) else {
    return Judged::unknown();
  };
  let mut sources = parsed.operands.clone();
  let target = match parsed.values(&TARGET).next() {
    Some(folder) => Some(folder.to_owned()),
    None => sources.pop(),
  };
  Judged::writing(Kind::Writes, target.into_iter().collect()).reading(sources)
}

/// A link is written at `-t <folder>`, its last operand, or with one operand in the folder the command
/// runs in; what it links to must lie inside.
fn link(args: &[String]) -> Judged {
  let Some(parsed) = options::parse(args, &LN) else {
    return Judged::unknown();
  };
  let mut targets = parsed.operands.clone();
  let name = match (parsed.values(&TARGET).next(), targets.len()) {
    (Some(folder), _) => folder.to_owned(),
    (None, 0 | 1) => ".".to_owned(),
    (None, _) => targets.pop().expect("two operands or more"),
  };
  let mut judged = Judged::writing(Kind::Writes, vec![name]);
  judged.inside = targets;
  judged
}

/// `find` lists unless it deletes what it finds; every other test or action must be one that only
/// tests or prints ([`known_safe`]), so `-exec`, `-fprint0` and the like are Unknown.
fn find(args: &[String]) -> Judged {
  let deletes = args.iter().any(|arg| arg == "-delete");
  let rest: Vec<String> = std::iter::once("find".to_owned())
    .chain(args.iter().filter(|arg| *arg != "-delete").cloned())
    .collect();
  let Some(words) = known_safe::readable_arguments(&rest) else {
    return Judged::unknown();
  };
  let read = words.into_iter().map(str::to_owned).collect();
  if !deletes {
    return Judged::of(Kind::ReadOnly).reading(read);
  }
  let roots: Vec<String> = args
    .iter()
    .take_while(|arg| !arg.starts_with(['-', '!', '(']))
    .cloned()
    .collect();
  let roots = if roots.is_empty() { vec![".".to_owned()] } else { roots };
  Judged::writing(Kind::Writes, roots).deleting().reading(read)
}

/// `sed` prints unless `-i` edits its files in place. A script that may write a file (`w`), read one
/// (`r`) or run a command (`e`) is not read: any script with those letters is Unknown.
fn sed(args: &[String]) -> Judged {
  let operands = operands(args);
  let script_given = has_option(args, &["-e", "--expression", "-f", "--file"]);
  let (script, files) = match (script_given, operands.split_first()) {
    (false, Some((script, files))) => (Some(script.as_str()), files.to_vec()),
    (..) => (None, operands.clone()),
  };
  if script_given || script.is_some_and(|script| script.contains(['w', 'W', 'e', 'r', 'R'])) {
    return Judged::unknown();
  }
  if has_option(args, &["-i", "--in-place"]) {
    return Judged::writing(Kind::Writes, files);
  }
  Judged::of(Kind::ReadOnly)
}
