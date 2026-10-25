//! Programs that only read, under any option and argument (ORC-95): they write no file, start no other program, set
//! nothing and reach no network, whatever they are given. A part of one that Orchester could not read to the end (an
//! option the tables do not list, `xargs` handing it more arguments) is still a read, which auto mode's reviewer may
//! judge in a step whose network is off or that only reads ([`crate::run::review`]): every path its words name is
//! placed all the same, so a credential location still holds it. `sort` (`-o`), `uniq` (a second file), `tree` (`-o`),
//! `file` (`-C`), `sed`, `awk`, `find` and `rg` (`--pre`) can write or start a program with some option, and `jq`
//! reads the environment: none is here.

use crate::permission::known_safe::program_name;

const ONLY_READS: [&str; 30] = [
  "ls",
  "cat",
  "head",
  "tail",
  "wc",
  "grep",
  "egrep",
  "fgrep",
  "cut",
  "nl",
  "tr",
  "cmp",
  "diff",
  "du",
  "df",
  "stat",
  "echo",
  "pwd",
  "basename",
  "dirname",
  "realpath",
  "readlink",
  "md5sum",
  "sha1sum",
  "sha256sum",
  "column",
  "which",
  "get-content",
  "get-childitem",
  "select-string",
];

/// Whether `program` (as [`super::program_key`] gives it) with `args` only reads: one of [`ONLY_READS`], or `xargs`
/// with listed options running one of them.
pub(super) fn only_reads(program: &str, args: &[String]) -> bool {
  if program == "xargs" {
    return xargs_program(args)
      .and_then(program_name)
      .is_some_and(|program| ONLY_READS.contains(&program.as_str()));
  }
  ONLY_READS.contains(&program)
}

/// `xargs`' options that take no value: the input's separators, skipping an empty input, tracing, the size check.
const XARGS_FLAGS: &str = "0rtx";
/// `xargs`' options with a value, attached or the next argument: how many arguments or lines per run, the
/// replacement string, parallel runs, the line size, the delimiter, the file the arguments come from.
const XARGS_VALUED: &str = "nILPsda";
const XARGS_LONG: [&str; 9] = [
  "null",
  "no-run-if-empty",
  "verbose",
  "exit",
  "max-args",
  "max-lines",
  "max-procs",
  "max-chars",
  "delimiter",
];

/// The program `xargs` runs, after its options; `None` with an option that is not listed (`-p` asks on the
/// terminal, `-o` reopens it) or without a program (`xargs` alone runs `echo`).
fn xargs_program(args: &[String]) -> Option<&str> {
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    if arg == "--" {
      return args.next().map(String::as_str);
    }
    if let Some(long) = arg.strip_prefix("--") {
      let name = long.split_once('=').map_or(long, |(name, _)| name);
      if !XARGS_LONG.contains(&name) {
        return None;
      }
      continue;
    }
    let Some(cluster) = arg.strip_prefix('-').filter(|cluster| !cluster.is_empty()) else {
      return Some(arg);
    };
    for (at, letter) in cluster.char_indices() {
      if XARGS_VALUED.contains(letter) {
        if cluster[at + letter.len_utf8()..].is_empty() {
          args.next()?;
        }
        break;
      }
      if !XARGS_FLAGS.contains(letter) {
        return None;
      }
    }
  }
  None
}
