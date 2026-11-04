//! The arguments of a search that are its pattern, not a path (ORC-612): `rg`, `grep` and `git grep` take a regular
//! expression whose backslashes (`\bword`, `\(`) would otherwise be placed as a Windows path rooted at the drive. A
//! pattern is told only when every option is one the search's grammar knows: an option it does not know could take
//! the next argument as its value, so a path could be taken for the pattern, and then every argument is placed.
//! Options are read wherever they stand, as `rg` and GNU `grep` read them; a search that stops at its first operand
//! only reads more of its arguments as paths that way, never fewer.

/// How one search reads its options.
struct Grammar {
  /// Short options without a value.
  flags: &'static str,
  /// Short options whose value is the rest of their cluster or the next argument.
  valued: &'static str,
  /// Short options whose value only attaches (`-C3`): written alone, builds differ on taking the next argument.
  attached: &'static str,
  /// `-NUM` is an option (`grep -3`).
  digits: bool,
  long_flags: &'static [&'static str],
  /// Long options whose value follows `=` or is the next argument.
  long_valued: &'static [&'static str],
  /// Long options whose value only follows `=`: written alone, builds differ on taking the next argument.
  long_attached: &'static [&'static str],
  /// The options whose value is a pattern.
  pattern: (char, &'static str),
  /// The options after which no argument is a pattern: patterns from a file, a listing of files.
  short_without_pattern: &'static str,
  long_without_pattern: &'static [&'static str],
  /// After `--` the pattern is the next argument (`rg`, `grep`); git takes `--` itself for the pattern.
  pattern_after_dashes: bool,
}

/// `rg` with the options the known-safe tables list ([`crate::permission::known_safe`]).
const RG: Grammar = Grammar {
  flags: ".0abcFHiIlnNopPqsSuUvwx",
  valued: "ABCdeEfgjmMrtT",
  attached: "",
  digits: false,
  long_flags: &[
    "files",
    "hidden",
    "no-ignore",
    "no-ignore-vcs",
    "no-ignore-parent",
    "no-ignore-dot",
    "no-config",
    "no-follow",
    "unrestricted",
    "ignore-case",
    "smart-case",
    "case-sensitive",
    "word-regexp",
    "line-regexp",
    "fixed-strings",
    "line-number",
    "no-line-number",
    "count",
    "count-matches",
    "files-with-matches",
    "files-without-match",
    "invert-match",
    "only-matching",
    "multiline",
    "multiline-dotall",
    "json",
    "heading",
    "no-heading",
    "with-filename",
    "no-filename",
    "column",
    "vimgrep",
    "byte-offset",
    "trim",
    "stats",
    "type-list",
    "glob-case-insensitive",
    "null",
    "quiet",
    "text",
    "binary",
    "crlf",
    "pcre2",
    "passthru",
    "no-messages",
  ],
  long_valued: &[
    "max-count",
    "max-depth",
    "max-filesize",
    "max-columns",
    "context",
    "after-context",
    "before-context",
    "sort",
    "sortr",
    "type",
    "type-not",
    "glob",
    "iglob",
    "regexp",
    "file",
    "ignore-file",
    "engine",
    "encoding",
    "threads",
    "replace",
    "color",
  ],
  long_attached: &[],
  pattern: ('e', "regexp"),
  short_without_pattern: "f",
  long_without_pattern: &["file", "files", "type-list"],
  pattern_after_dashes: true,
};

/// GNU and BSD `grep`. BSD's `-C` and `--context` take their value only attached, GNU's also as the next argument.
const GREP: Grammar = Grammar {
  flags: "abcEFGHhIiJLlMnOopqRrSsTUuVvwxyZz",
  valued: "efmABdD",
  attached: "C",
  digits: true,
  long_flags: &[
    "extended-regexp",
    "fixed-strings",
    "basic-regexp",
    "perl-regexp",
    "ignore-case",
    "no-ignore-case",
    "word-regexp",
    "line-regexp",
    "null-data",
    "no-messages",
    "invert-match",
    "byte-offset",
    "line-number",
    "line-buffered",
    "with-filename",
    "no-filename",
    "only-matching",
    "quiet",
    "silent",
    "text",
    "recursive",
    "files-without-match",
    "files-with-matches",
    "count",
    "initial-tab",
    "null",
    "color",
    "colour",
    "no-group-separator",
  ],
  long_valued: &[
    "regexp",
    "file",
    "max-count",
    "after-context",
    "before-context",
    "directories",
    "devices",
    "include",
    "exclude",
    "exclude-from",
    "exclude-dir",
    "label",
    "binary-files",
    "group-separator",
  ],
  long_attached: &["context"],
  pattern: ('e', "regexp"),
  short_without_pattern: "f",
  long_without_pattern: &["file"],
  pattern_after_dashes: true,
};

/// `git grep` with the options the known-safe tables list. `--color` takes its value only after `=`.
const GIT_GREP: Grammar = Grammar {
  flags: "aiIwvhHEGPFnlLzocpWqr",
  valued: "eABCm",
  attached: "",
  digits: false,
  long_flags: &[
    "cached",
    "untracked",
    "exclude-standard",
    "no-exclude-standard",
    "text",
    "no-textconv",
    "ignore-case",
    "recursive",
    "no-recursive",
    "word-regexp",
    "invert-match",
    "full-name",
    "extended-regexp",
    "basic-regexp",
    "perl-regexp",
    "fixed-strings",
    "line-number",
    "column",
    "files-with-matches",
    "name-only",
    "files-without-match",
    "null",
    "only-matching",
    "count",
    "color",
    "no-color",
    "break",
    "heading",
    "show-function",
    "function-context",
    "and",
    "or",
    "not",
    "all-match",
    "quiet",
  ],
  long_valued: &["max-depth", "context", "after-context", "before-context", "max-count"],
  long_attached: &[],
  pattern: ('e', "regexp"),
  short_without_pattern: "",
  long_without_pattern: &[],
  pattern_after_dashes: false,
};

/// Where the pattern of `program` with `args` stands among `args`; none for a program that is no search, or when
/// it cannot be told.
pub(super) fn positions(program: &str, args: &[String]) -> Vec<usize> {
  let grammar = match program {
    "rg" => RG,
    "grep" | "egrep" | "fgrep" => GREP,
    _ => return Vec::new(),
  };
  grammar.patterns(args).unwrap_or_default()
}

/// Where the pattern of `git grep` stands among `with_sub`, the subcommand first.
pub(super) fn git_grep(with_sub: &[String]) -> Vec<usize> {
  let patterns = with_sub.get(1..).and_then(|rest| GIT_GREP.patterns(rest));
  patterns.unwrap_or_default().into_iter().map(|at| at + 1).collect()
}

/// `args` without the arguments at `positions`.
pub(super) fn without(args: &[String], positions: &[usize]) -> Vec<String> {
  args
    .iter()
    .enumerate()
    .filter(|(at, _)| !positions.contains(at))
    .map(|(_, arg)| arg.clone())
    .collect()
}

/// What the options read so far say of the pattern.
#[derive(Default)]
struct Reading {
  /// The arguments that are patterns: an option's value, or a whole option with the value attached.
  patterns: Vec<usize>,
  /// An option gave a pattern or said there is none: no operand is one.
  given: bool,
}

impl Grammar {
  /// The positions of the patterns among `args`; `None` when an option is not known.
  fn patterns(&self, args: &[String]) -> Option<Vec<usize>> {
    let mut reading = Reading::default();
    let mut operands = Vec::new();
    let mut dashes = false;
    let mut at = 0;
    while let Some(arg) = args.get(at) {
      let next = at + 1;
      at = if dashes || arg == "-" || !arg.starts_with('-') {
        operands.push(at);
        next
      } else if arg == "--" {
        if !(self.pattern_after_dashes || reading.given || !operands.is_empty()) {
          return None;
        }
        dashes = true;
        next
      } else if let Some(long) = arg.strip_prefix("--") {
        self.long(long, at, &mut reading)?
      } else {
        self.short(&arg[1..], at, &mut reading)?
      };
    }
    if !reading.given {
      reading.patterns.extend(operands.first());
    }
    Some(reading.patterns)
  }

  /// Reads the long option `long` at `at`; where the next argument to read stands.
  fn long(&self, long: &str, at: usize, reading: &mut Reading) -> Option<usize> {
    let (name, attached) = long.split_once('=').map_or((long, false), |(name, _)| (name, true));
    let valued = self.long_valued.contains(&name);
    if !(valued || self.long_flags.contains(&name) || (attached && self.long_attached.contains(&name))) {
      return None;
    }
    reading.given |= self.long_without_pattern.contains(&name);
    let value = match (valued, attached) {
      (true, false) => at + 1,
      _ => at,
    };
    if name == self.pattern.1 {
      reading.patterns.push(value);
      reading.given = true;
    }
    Some(if value == at { at + 1 } else { at + 2 })
  }

  /// Reads the short-option cluster `cluster` at `at`; where the next argument to read stands.
  fn short(&self, cluster: &str, at: usize, reading: &mut Reading) -> Option<usize> {
    for (offset, letter) in cluster.char_indices() {
      if (self.digits && letter.is_ascii_digit()) || self.flags.contains(letter) {
        reading.given |= self.short_without_pattern.contains(letter);
        continue;
      }
      let attached = offset + letter.len_utf8() < cluster.len();
      if !(self.valued.contains(letter) || (attached && self.attached.contains(letter))) {
        return None;
      }
      reading.given |= self.short_without_pattern.contains(letter);
      let value = if attached { at } else { at + 1 };
      if letter == self.pattern.0 {
        reading.patterns.push(value);
        reading.given = true;
      }
      return Some(value + 1);
    }
    Some(at + 1)
  }
}
