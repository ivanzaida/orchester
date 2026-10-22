//! What the task's record list says when its filters show nothing (`n3lxin` in `fQ33n`): that nothing matches, how
//! search works, what matches among other types or statuses with a way to show them, and *Clear search*. The
//! project's page says it per tab (`tab_no_match`).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::memory::Memory;
use orchester_workspace::memory::listing::{self, Filters, StatusFilter};

use super::{centered::Centered, state::MemoryState, words};
use crate::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
};

pub fn no_match(ctx: &Ctx, state: &MemoryState, records: &[Memory], filters: &Filters) -> Element {
  let lexical = ctx.t("memory.noMatch.lexical");
  let elsewhere = elsewhere(ctx, state, records, filters);
  let explanation = match &elsewhere.explanation {
    Some(more) => format!("{lexical} {more}"),
    None => lexical.to_string(),
  };
  let mut empty = Centered::new("memory-no-match", Icon::SearchX, title(ctx, filters), explanation);
  if let Some(show) = elsewhere.show {
    empty = empty.action(show);
  }
  if !filters.search.trim().is_empty() {
    let search = state.search.clone();
    empty = empty.action(
      ActionButton::new(
        "memory-clear-search",
        ButtonKind::Ghost,
        ctx.t("memory.toolbar.clearSearch"),
      )
      .icon(Icon::X)
      .on_click(move || search.set(String::new())),
    );
  }
  empty.into()
}

/// *No decisions match “retry”*, *No records match “retry”*, or *No records match these filters* without a search.
fn title(ctx: &Ctx, filters: &Filters) -> Arc<str> {
  let query = filters.search.trim();
  match (query.is_empty(), filters.kind) {
    (false, Some(kind)) => ctx.t_args(
      "memory.noMatch.kindSearch",
      [
        ("kind", words::kind_plural(ctx, kind).to_string()),
        ("query", query.to_owned()),
      ],
    ),
    (false, None) => ctx.t_args("memory.noMatch.search", [("query", query)]),
    (true, _) => ctx.t("memory.noMatch.filters"),
  }
}

/// What matches outside the filters: words after the explanation, and a way to show it.
#[derive(Default)]
struct Elsewhere {
  explanation: Option<String>,
  show: Option<ActionButton>,
}

fn elsewhere(ctx: &Ctx, state: &MemoryState, records: &[Memory], filters: &Filters) -> Elsewhere {
  let other_types = listing::found_elsewhere(records, filters);
  let (other_statuses, _) = listing::found_in_other_statuses(records, filters);
  let show = |id: &str, label: Arc<str>, change: Box<dyn Fn(&mut Filters) + Send + Sync>| {
    let filters = state.filters.clone();
    ActionButton::new(id, ButtonKind::Secondary, label)
      .icon(Icon::List)
      .on_click(move || filters.update(|filters| change(filters)))
  };
  if other_types > 0 {
    let explanation = match other_types {
      1 => ctx.t("memory.noMatch.otherTypesOne"),
      count => ctx.t_args("memory.noMatch.otherTypes", [("count", count.to_string())]),
    };
    return Elsewhere {
      explanation: Some(explanation.to_string()),
      show: Some(show(
        "memory-show-all-types",
        ctx.t("memory.noMatch.showAllTypes"),
        Box::new(|filters| filters.kind = None),
      )),
    };
  }
  if other_statuses > 0 {
    let explanation = match other_statuses {
      1 => ctx.t("memory.noMatch.otherStatusesOne"),
      count => ctx.t_args("memory.noMatch.otherStatuses", [("count", count.to_string())]),
    };
    return Elsewhere {
      explanation: Some(explanation.to_string()),
      show: Some(show(
        "memory-show-all-statuses",
        ctx.t("memory.noMatch.showAllStatuses"),
        Box::new(|filters| filters.status = StatusFilter::All),
      )),
    };
  }
  Elsewhere::default()
}
