//! Programs that create, copy, move, link or delete the files they name, and `find`. `cp`, `ln`,
//! `install` and `mv` are read against the options they are known to take ([`options`]), so a target
//! written as an option (`--target-directory=…`) is never missed; a link's target must lie inside, as a
//! link to a folder outside would let later commands reach it through the worktree.

use super::{
  options::{self, Known, Takes},
  programs::{Judged, Kind, operands},
};
use crate::permission::known_safe;

/// Programs that write each file they name: every argument that is not an option.
const WRITES_ARGS: [&str; 6] = ["mkdir", "touch", "chmod", "chown", "tee", "truncate"];

/// Programs of [`WRITES_ARGS`] that only create or change the files they name, as an edit does; `chmod`,
/// `chown` and `truncate` change what an edit does not.
const EDITS_ARGS: [&str; 3] = ["mkdir", "touch", "tee"];

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
    "cp" => copy_files(args),
    "install" if options::parse(args, &INSTALL).is_some_and(|parsed| parsed.has(&["-d", "--directory"])) => {
      Judged::writing(Kind::Writes, operands(args))
    }
    "install" => copy(args, &INSTALL),
    "ln" => link(args),
    "mv" => options::parse(args, &MV).map_or_else(Judged::unknown, |parsed| {
      let mut written = parsed.operands.clone();
      let mut sources = parsed.operands.clone();
      if parsed.values(&TARGET).next().is_none() {
        sources.pop();
      }
      written.extend(parsed.values(&TARGET).map(str::to_owned));
      Judged::writing(Kind::Writes, written)
        .deleting()
        .moving(sources)
        .editing()
    }),
    "find" => find(args),
    program if EDITS_ARGS.contains(&program) => Judged::writing(Kind::Writes, operands(args)).editing(),
    program if WRITES_ARGS.contains(&program) => Judged::writing(Kind::Writes, operands(args)),
    program if DELETES_ARGS.contains(&program) => Judged::writing(Kind::Writes, operands(args)).deleting().editing(),
    _ => return None,
  })
}

/// `cp` options that copy a folder's tree or a link as it is (`-r`, `-a`, `-P`, `-d`): what they bring
/// to the target can be links that lead anywhere, which a later write through the copy would follow.
const COPIES_TREES: [&str; 8] = [
  "-r",
  "-R",
  "-a",
  "-P",
  "-d",
  "--recursive",
  "--archive",
  "--no-dereference",
];

/// `cp`: an edit when it copies files as they are named, not a tree or a link as it is.
fn copy_files(args: &[String]) -> Judged {
  let judged = copy(args, &CP);
  let sources = judged.reads.clone();
  let judged = judged.moving(sources);
  if options::parse(args, &CP).is_some_and(|parsed| parsed.has(&COPIES_TREES)) {
    judged
  } else {
    judged.editing()
  }
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
    return Judged::of(Kind::ReadOnly).known_safe().reading(read);
  }
  let roots: Vec<String> = args
    .iter()
    .take_while(|arg| !arg.starts_with(['-', '!', '(']))
    .cloned()
    .collect();
  let roots = if roots.is_empty() { vec![".".to_owned()] } else { roots };
  Judged::writing(Kind::Writes, roots).deleting().reading(read)
}
