//! macOS's code-signing queries, which only read: `security find-identity` lists the signing identities of the
//! keychains (their certificates' names and hashes, no private key), and `spctl --assess` tells whether Gatekeeper
//! would let the files it names run. Any other form of either is left to [`super::programs`]: `security`'s other
//! commands read a credential store, `spctl`'s others change the system's policy.

use super::programs::{Judged, Kind};

/// `program` with `args` when it is such a query, with the files it names; `None` for any other form.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  match (program, args.split_first()) {
    ("security", Some((sub, rest))) if sub == "find-identity" => {
      find_identity(rest).map(|judged| judged.subcommand(0, sub.clone()))
    }
    ("spctl", _) => assess(args),
    _ => None,
  }
}

/// `security find-identity [-v] [-p <policy>] [-s <string>] [<keychain>…]`.
fn find_identity(rest: &[String]) -> Option<Judged> {
  let mut keychains = Vec::new();
  let mut words = rest.iter();
  while let Some(word) = words.next() {
    match word.as_str() {
      "-v" => {}
      "-p" | "-s" => {
        words.next()?;
      }
      word if !word.starts_with('-') => keychains.push(word.to_owned()),
      _ => return None,
    }
  }
  Some(reading(keychains))
}

/// `spctl -a [-v…] [-t <type>] [--context <context>] [--raw] [--ignore-cache] <path>…`, each option a word of its own.
fn assess(args: &[String]) -> Option<Judged> {
  let mut assesses = false;
  let mut paths = Vec::new();
  let mut words = args.iter();
  while let Some(word) = words.next() {
    match word.as_str() {
      "-a" | "--assess" => assesses = true,
      "--verbose" | "--raw" | "--ignore-cache" => {}
      "-t" | "--type" | "--context" => {
        words.next()?;
      }
      word
        if word
          .strip_prefix('-')
          .is_some_and(|flags| !flags.is_empty() && flags.chars().all(|c| c == 'v')) => {}
      word if !word.starts_with('-') => paths.push(word.to_owned()),
      _ => return None,
    }
  }
  (assesses && !paths.is_empty()).then(|| reading(paths))
}

/// A read of `files`, whose other words name no file.
fn reading(files: Vec<String>) -> Judged {
  Judged::of(Kind::ReadOnly).naming_no_file().reading(files)
}
