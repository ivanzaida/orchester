//! Similar titles (`docs/architecture/server.md`, "Review before joining"): a hint, never a merge,
//! that a local task may duplicate one the server project has already.

use std::collections::BTreeSet;

/// Two titles look alike when this share (numerator, denominator) of the shorter title's words is
/// in both…
const SHARED_SHARE: (usize, usize) = (3, 5);
/// …and at least this many words: fewer in common is chance, however short the titles are.
const MIN_COMMON: usize = 2;

/// The positions in `others` of the titles similar to `title`, the most similar first (ties in the
/// order given). Words are case-folded runs of letters and digits; two titles are similar when at
/// least two words, and three fifths of the shorter title's words, are in both.
pub fn similar_titles<'a>(title: &str, others: impl IntoIterator<Item = &'a str>) -> Vec<usize> {
  let ours = words(title);
  let mut found: Vec<(usize, usize, usize)> = others
    .into_iter()
    .enumerate()
    .filter_map(|(index, other)| {
      let theirs = words(other);
      let common = ours.intersection(&theirs).count();
      let shorter = ours.len().min(theirs.len());
      let (numerator, denominator) = SHARED_SHARE;
      (common >= MIN_COMMON && common * denominator >= shorter * numerator).then_some((index, common, shorter))
    })
    .collect();
  // The larger share first: a/b > c/d as a·d > c·b.
  found.sort_by(|a, b| (b.1 * a.2).cmp(&(a.1 * b.2)));
  found.into_iter().map(|(index, ..)| index).collect()
}

fn words(title: &str) -> BTreeSet<String> {
  title
    .split(|c: char| !c.is_alphanumeric())
    .filter(|word| !word.is_empty())
    .map(str::to_lowercase)
    .collect()
}
