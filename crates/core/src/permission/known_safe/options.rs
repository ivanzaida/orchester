//! The options a known-safe program may get, and how its arguments are read against them, the way
//! `getopt_long` and git's parse-options read them.

/// The options one program (or git subcommand) accepts in a known-safe command. Every other option is
/// refused, an abbreviation of a listed one too: programs match an exact name before an abbreviation,
/// and an abbreviation could stand for an option that is not listed.
pub(super) struct Options {
  /// Long options by their exact name without `--`, also written `--name=value`.
  pub(super) long: &'static [&'static [&'static str]],
  /// Short options that take no value; a cluster may combine them (`-la`).
  pub(super) flags: &'static str,
  /// Short options whose value is the rest of their cluster (`-n20`). Written alone, their value is the
  /// next argument, which is then read like any other argument: as an option when it starts with `-`.
  pub(super) valued: &'static str,
}

impl Options {
  /// The parts of `args` that may name a file, or `None` when an option is not listed: every argument,
  /// the value after `=` of a long option, and the value attached in a short-option cluster. After
  /// `--` every argument is an operand.
  pub(super) fn readable<'a>(&self, args: &'a [String]) -> Option<Vec<&'a str>> {
    let mut candidates = Vec::new();
    let mut operands = false;
    for arg in args {
      candidates.push(arg.as_str());
      if operands || arg == "-" || !arg.starts_with('-') {
        continue;
      }
      if arg == "--" {
        operands = true;
      } else if let Some(long) = arg.strip_prefix("--") {
        let (name, value) = long
          .split_once('=')
          .map_or((long, None), |(name, value)| (name, Some(value)));
        if !self.long.iter().any(|group| group.contains(&name)) {
          return None;
        }
        candidates.extend(value);
      } else if !self.cluster(&arg[1..], &mut candidates) {
        return None;
      }
    }
    Some(candidates)
  }

  /// Whether every letter of the short-option `cluster` is listed; a value attached to a valued letter
  /// joins `candidates`.
  fn cluster<'a>(&self, cluster: &'a str, candidates: &mut Vec<&'a str>) -> bool {
    for (at, letter) in cluster.char_indices() {
      if self.valued.contains(letter) {
        let value = &cluster[at + letter.len_utf8()..];
        if !value.is_empty() {
          candidates.push(value);
        }
        return true;
      }
      if !self.flags.contains(letter) {
        return false;
      }
    }
    true
  }
}
