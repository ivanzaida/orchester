//! `head` and `tail`: their options, and the counts they take. GNU `tail` reads an obsolete form in
//! which a count that ends with `f` follows the file until the command is stopped (`tail +1f a`,
//! `tail -cf a`), so every count must be a number and no argument may have that form.

use super::options::Options;

/// `head` and `tail`, also with a count alone (`-20`); `tail` without `-f`, which waits for more.
const OPTIONS: Options = Options {
  long: &[&["lines", "bytes", "quiet", "silent", "verbose", "zero-terminated"]],
  flags: "0123456789qvz",
  valued: "nc",
};

/// The size suffixes GNU `head` and `tail` accept after a count.
const SUFFIXES: [&str; 11] = ["", "b", "kB", "K", "KiB", "MB", "M", "MiB", "GB", "G", "GiB"];

/// Where the count of an option is.
enum Count<'a> {
  /// Attached to the option: `-n20`, `--lines=20`.
  Attached(&'a str),
  /// The next argument: `-n 20`, `--lines 20`.
  Next,
  /// The argument takes no count.
  Absent,
}

/// The arguments of `head` or `tail` that may name a file, or `None` when an option is not listed, a
/// count is not a number, or an argument has the obsolete follow form.
pub(super) fn readable(args: &[String]) -> Option<Vec<&str>> {
  if args.iter().any(|arg| follows(arg)) {
    return None;
  }
  let mut expects_count = false;
  for arg in args {
    if std::mem::take(&mut expects_count) {
      if !is_count(arg) {
        return None;
      }
      continue;
    }
    if arg == "--" {
      break;
    }
    match count(arg) {
      Count::Attached(value) if !is_count(value) => return None,
      Count::Next => expects_count = true,
      Count::Attached(_) | Count::Absent => {}
    }
  }
  OPTIONS.readable(args)
}

fn count(arg: &str) -> Count<'_> {
  if let Some(long) = arg.strip_prefix("--") {
    return match long.split_once('=') {
      Some(("lines" | "bytes", value)) => Count::Attached(value),
      None if long == "lines" || long == "bytes" => Count::Next,
      _ => Count::Absent,
    };
  }
  let Some(cluster) = arg.strip_prefix('-') else {
    return Count::Absent;
  };
  match cluster.find(['n', 'c']) {
    Some(at) if at + 1 == cluster.len() => Count::Next,
    Some(at) => Count::Attached(&cluster[at + 1..]),
    None => Count::Absent,
  }
}

/// Whether `value` is a count: an optional sign, digits, and an optional size suffix.
fn is_count(value: &str) -> bool {
  let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
  let digits = unsigned.find(|c: char| !c.is_ascii_digit()).unwrap_or(unsigned.len());
  digits > 0 && SUFFIXES.contains(&&unsigned[digits..])
}

/// Whether `arg` has GNU's obsolete follow form: a sign, digits, an optional unit, then `f`.
fn follows(arg: &str) -> bool {
  arg.strip_prefix(['+', '-']).is_some_and(|rest| {
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    rest.strip_prefix(['b', 'c', 'l']).unwrap_or(rest).starts_with('f')
  })
}
