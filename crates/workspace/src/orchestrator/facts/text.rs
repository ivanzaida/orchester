//! What a draft changes in an instruction's or skill's text, line by line (*What changes* in `b55NqO`): the lines it
//! removes and adds, each with a line around them, as the diff lines show them.

/// How a line of the text changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineChange {
  Same,
  Removed,
  Added,
}

/// One line shown: its numbers in the text before and after (1-based), how it changes, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLine {
  pub old: Option<usize>,
  pub new: Option<usize>,
  pub change: LineChange,
  pub text: String,
}

/// The lines a draft removes and adds, and the lines shown of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextDiff {
  pub removed: usize,
  pub added: usize,
  pub lines: Vec<TextLine>,
  /// More lines changed than a card shows: the rest is read in the draft.
  pub cut: bool,
}

/// Unchanged lines shown on each side of a change.
const AROUND: usize = 1;
/// Lines a card shows at most.
const SHOWN: usize = 40;
/// The largest comparison made: lines before times lines after. A longer text is not compared here.
const MOST_PAIRS: usize = 1_000_000;

/// What `after` changes in `before`; `None` when nothing changes or the texts are too long to compare here.
pub(super) fn diff(before: &str, after: &str) -> Option<TextDiff> {
  let (old, new): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
  if old == new || (old.len() + 1).saturating_mul(new.len() + 1) > MOST_PAIRS {
    return None;
  }
  let all = lines(&old, &new);
  let removed = all.iter().filter(|line| line.change == LineChange::Removed).count();
  let added = all.iter().filter(|line| line.change == LineChange::Added).count();
  let near = |index: usize| {
    let from = index.saturating_sub(AROUND);
    let to = (index + AROUND).min(all.len() - 1);
    all[from..=to].iter().any(|line| line.change != LineChange::Same)
  };
  let kept: Vec<TextLine> = (0..all.len())
    .filter(|index| near(*index))
    .map(|index| all[index].clone())
    .collect();
  let cut = kept.len() > SHOWN;
  Some(TextDiff {
    removed,
    added,
    lines: kept.into_iter().take(SHOWN).collect(),
    cut,
  })
}

/// Every line of `old` and `new` in order, by their longest common sequence of lines.
fn lines(old: &[&str], new: &[&str]) -> Vec<TextLine> {
  let width = new.len() + 1;
  // `common[i * width + j]`: the longest common sequence of `old[i..]` and `new[j..]`.
  let mut common = vec![0_u32; (old.len() + 1) * width];
  for i in (0..old.len()).rev() {
    for j in (0..new.len()).rev() {
      common[i * width + j] = if old[i] == new[j] {
        common[(i + 1) * width + j + 1] + 1
      } else {
        common[(i + 1) * width + j].max(common[i * width + j + 1])
      };
    }
  }
  let line = |old: Option<usize>, new: Option<usize>, change, text: &str| TextLine {
    old: old.map(|index| index + 1),
    new: new.map(|index| index + 1),
    change,
    text: text.to_owned(),
  };
  let (mut i, mut j, mut all) = (0, 0, Vec::new());
  while i < old.len() || j < new.len() {
    if i < old.len() && j < new.len() && old[i] == new[j] {
      all.push(line(Some(i), Some(j), LineChange::Same, old[i]));
      (i, j) = (i + 1, j + 1);
    } else if j < new.len() && (i == old.len() || common[i * width + j + 1] >= common[(i + 1) * width + j]) {
      all.push(line(None, Some(j), LineChange::Added, new[j]));
      j += 1;
    } else {
      all.push(line(Some(i), None, LineChange::Removed, old[i]));
      i += 1;
    }
  }
  // As a diff reads: what a change removes before what it adds.
  order_removed_first(&mut all);
  all
}

/// Within each run of changed lines, the removed lines before the added ones.
fn order_removed_first(all: &mut [TextLine]) {
  let mut start = 0;
  while start < all.len() {
    if all[start].change == LineChange::Same {
      start += 1;
      continue;
    }
    let end = (start..all.len())
      .find(|index| all[*index].change == LineChange::Same)
      .unwrap_or(all.len());
    all[start..end].sort_by_key(|line| line.change == LineChange::Added);
    start = end;
  }
}
