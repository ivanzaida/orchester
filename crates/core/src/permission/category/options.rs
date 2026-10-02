//! Reading a program's arguments against the options it is known to take, the way `getopt_long` reads
//! them: `--name value` or `--name=value`, short options alone or in a cluster (`-sSL`), a short
//! option's value attached (`-ofile`) or the next argument, and single-dash words of their own (`-nv`).
//! An option the table does not list makes the whole command unread: it could name a file or start a
//! program, so the caller classifies the command as Unknown.

/// What an option does with its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Takes {
  /// No value.
  Nothing,
  /// A value that names no file.
  Value,
  /// A file it reads.
  Reads,
  /// A file it writes.
  Writes,
}

/// One option as written (`-o`, `--output`, `-nv`) and what it takes.
pub(super) type Known = (&'static str, Takes);

/// The arguments, read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Parsed {
  pub(super) operands: Vec<String>,
  /// The options given, by their name in the table, with their values.
  pub(super) options: Vec<(&'static str, Option<String>)>,
  /// The values of options that read a file.
  pub(super) reads: Vec<String>,
  /// The values of options that write a file.
  pub(super) writes: Vec<String>,
}

impl Parsed {
  /// Whether one of the options `names` was given.
  pub(super) fn has(&self, names: &[&str]) -> bool {
    self.options.iter().any(|(name, _)| names.contains(name))
  }

  /// The values given to the options `names`.
  pub(super) fn values<'a>(&'a self, names: &'a [&str]) -> impl Iterator<Item = &'a str> {
    self
      .options
      .iter()
      .filter(move |(name, _)| names.contains(name))
      .filter_map(|(_, value)| value.as_deref())
  }

  fn take(&mut self, (name, takes): Known, value: Option<String>) {
    match (takes, &value) {
      (Takes::Reads, Some(value)) => self.reads.push(value.clone()),
      (Takes::Writes, Some(value)) => self.writes.push(value.clone()),
      _ => {}
    }
    self.options.push((name, value));
  }
}

/// `args` read against `table`; `None` when an option is not listed, a listed one gets a value it does
/// not take, or a value is missing.
pub(super) fn parse(args: &[String], table: &[Known]) -> Option<Parsed> {
  let mut parsed = Parsed::default();
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    if arg == "--" {
      parsed.operands.extend(args.by_ref().cloned());
    } else if arg == "-" || !arg.starts_with('-') {
      parsed.operands.push(arg.clone());
    } else if let Some(long) = arg.strip_prefix("--") {
      let (name, value) = long
        .split_once('=')
        .map_or((long, None), |(name, value)| (name, Some(value.to_owned())));
      let known = lookup(table, "--", name)?;
      let value = match (known.1, value) {
        (Takes::Nothing, Some(_)) => return None,
        (Takes::Nothing, None) => None,
        (_, Some(value)) => Some(value),
        (_, None) => Some(args.next()?.clone()),
      };
      parsed.take(known, value);
    } else if let Some(known) = lookup(table, "", arg) {
      let value = match known.1 {
        Takes::Nothing => None,
        _ => Some(args.next()?.clone()),
      };
      parsed.take(known, value);
    } else {
      cluster(&arg[1..], table, &mut args, &mut parsed)?;
    }
  }
  Some(parsed)
}

/// The short options of `cluster` (without its `-`); the first that takes a value takes the rest of
/// the cluster, or the next argument.
fn cluster<'a>(
  cluster: &str,
  table: &[Known],
  args: &mut impl Iterator<Item = &'a String>,
  parsed: &mut Parsed,
) -> Option<()> {
  for (at, letter) in cluster.char_indices() {
    let known = lookup(table, "-", &letter.to_string())?;
    if known.1 == Takes::Nothing {
      parsed.take(known, None);
      continue;
    }
    let rest = &cluster[at + letter.len_utf8()..];
    let value = if rest.is_empty() {
      args.next()?.clone()
    } else {
      rest.to_owned()
    };
    parsed.take(known, Some(value));
    return Some(());
  }
  Some(())
}

fn lookup(table: &[Known], dashes: &str, name: &str) -> Option<Known> {
  table.iter().copied().find(|(known, _)| {
    known
      .strip_prefix(dashes)
      .is_some_and(|rest| rest == name && (dashes != "-" || !rest.starts_with('-')))
  })
}
