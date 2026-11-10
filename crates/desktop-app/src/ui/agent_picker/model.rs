//! What the Agent picker lists and the rules over it as pure state: the items as its caller gives them, how Search
//! filters them, the rows they make (a heading for each group that has an item) and where the keys go.

use std::sync::Arc;

use super::{GROUP_HEIGHT, ROW_HEIGHT};
use crate::ui::{agent_mark::AgentMark, icon::Icon};

/// The two groups of the list, in the order it shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
  Profiles,
  Tools,
}

impl Group {
  const ORDER: [Self; 2] = [Self::Profiles, Self::Tools];
}

/// An item's mark: its CLI's maker, or a glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
  Agent(AgentMark),
  Icon(Icon),
}

/// One agent profile or tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
  /// What the picker hands back when the item is chosen.
  pub key: Arc<str>,
  pub group: Group,
  pub mark: Mark,
  pub name: Arc<str>,
  /// What it runs on: a profile's CLI, connection and model; a tool's command and that its exit code decides.
  pub runs_on: Arc<str>,
  /// Why it is not ready, said in place of `runs_on`; it can still be chosen.
  pub warning: Option<Arc<str>>,
}

/// The items whose name or what they run on holds `query` (case does not matter), as places in `items`, in the
/// list's order: the profiles first, then the tools, each group in the order it was given.
pub fn filter(items: &[Item], query: &str) -> Vec<usize> {
  let wanted = query.trim().to_lowercase();
  Group::ORDER
    .into_iter()
    .flat_map(|group| {
      let wanted = wanted.clone();
      items
        .iter()
        .enumerate()
        .filter(move |(_, item)| {
          item.group == group
            && (item.name.to_lowercase().contains(&wanted) || item.runs_on.to_lowercase().contains(&wanted))
        })
        .map(|(at, _)| at)
    })
    .collect()
}

/// One row of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
  Heading(Group),
  /// The place in `items` of the item, and its place among the items shown (where the keys count).
  Item {
    at: usize,
    shown: usize,
  },
}

/// The rows of the items `found` (as [`filter`] returned them): a heading before the first item of each group.
pub fn rows(items: &[Item], found: &[usize]) -> Vec<Row> {
  let mut rows = Vec::new();
  let mut last = None;
  for (shown, &at) in found.iter().enumerate() {
    let group = items[at].group;
    if last != Some(group) {
      rows.push(Row::Heading(group));
      last = Some(group);
    }
    rows.push(Row::Item { at, shown });
  }
  rows
}

/// Where the item shown at `shown` is in the list: how far down its row starts and how high it is. The first item of a
/// group takes its heading with it, so that scrolling to it shows which group it is in.
pub fn extent(rows: &[Row], shown: usize) -> Option<(f32, f32)> {
  let mut top = 0.0;
  let mut heading = None;
  for row in rows {
    match row {
      Row::Heading(_) => {
        heading = Some(top);
        top += GROUP_HEIGHT;
      }
      Row::Item { shown: at, .. } if *at == shown => {
        let start = heading.unwrap_or(top);
        return Some((start, top + ROW_HEIGHT - start));
      }
      Row::Item { .. } => {
        heading = None;
        top += ROW_HEIGHT;
      }
    }
  }
  None
}

/// Where ↑ and ↓ put the keys, which stop at the ends: one step from where they are; the first press shows where they
/// start, the chosen item, else the first. `None` when there is nothing to move over.
pub fn moved(at: Option<usize>, chosen: Option<usize>, count: usize, by: isize) -> Option<usize> {
  let last = count.checked_sub(1)?;
  Some(match at {
    Some(from) => from.saturating_add_signed(by).min(last),
    None => chosen.unwrap_or(0).min(last),
  })
}

/// Where the keys are: where an arrow put them, else the first item found while Search holds text; always inside the
/// `count` items shown.
pub fn keys_at(at: Option<usize>, searched: bool, count: usize) -> Option<usize> {
  let last = count.checked_sub(1)?;
  at.or(searched.then_some(0)).map(|at| at.min(last))
}
