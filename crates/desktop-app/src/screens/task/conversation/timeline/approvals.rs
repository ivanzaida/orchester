//! Requests Autopilot approved one after another in a step (04 `H6kPd`: three *Approved by autopilot* rows, the
//! last *· 15:19 · 3 more*): each a quiet row (`cards::quiet_line`), 10 apart as the design's transcript rows are
//! drawn here in one row of the list. Folded, the first [`SHOWN`] show and the last of them says how many more
//! follow; its press opens them all, and the person's choice is kept while the timeline grows
//! (`docs/design/approval-modes.md`, open question 3).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tasks::timeline::{Item, ItemKind};

use super::Timeline;
use crate::{screens::task::cards::quiet_line, theme::ExtraSpacing, ui::pressable::Press};

/// The Autopilot rows a folded run of them shows (`vs9mB`, `ZlDid`, `FHr3B` in `H6kPd`).
pub(super) const SHOWN: usize = 3;

/// The rows `key` of `items`, all of them while `open`.
pub(super) fn rows(ctx: &Ctx, timeline: &Timeline, (key, items): (&Arc<str>, &[Item]), open: bool) -> Element {
  let shown = if open { items.len() } else { items.len().min(SHOWN) };
  let more = items.len() - shown;
  let lines = items[..shown].iter().enumerate().filter_map(|(index, item)| {
    let ItemKind::Approval(card) = &item.kind else {
      return None;
    };
    let last = index + 1 == shown;
    let press = (last && more > 0).then(|| {
      let (toggles, key) = (timeline.toggles.clone(), key.clone());
      (more, Press::run(move || toggles.flip(&key, false)))
    });
    Some(quiet_line(
      ctx,
      &format!("cards-approval-{}", card.request.id),
      card,
      press,
    ))
  });
  Column::new()
    .id(format!("conversation-{key}"))
    .describe("count", items.len().to_string())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(lines)
    .into()
}
