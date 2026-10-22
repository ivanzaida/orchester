//! What a status tab of the project's page says when its search and filters show nothing (`KCGyF` in `C0OJpC`):
//! that nothing in this tab matches (*No active records match “oauth refresh”*), how search works, what matches among
//! other types here with *Show all types*, else which other tabs hold matches (*1 match in Deprecated*; several tabs:
//! *1 in Deprecated · 2 in Rejected*) with a button that opens each with the search kept, and *Clear search*.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::memory::{
  listing::{self, Filters},
  read::ProjectMemory,
};

use super::{centered::Centered, state::MemoryState, tabs, words};
use crate::{
  navigation::Navigation,
  routes::MemoryTab,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// `tab` of `memory` with nothing that `filters` show, which are the page's filters with the tab's statuses.
pub fn tab_no_match(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, tab, order): (&ProjectMemory, MemoryTab, tabs::Order),
  filters: &Filters,
) -> Element {
  let records = memory.pool(filters.status);
  let mut empty = Centered::new(
    "memory-no-match",
    Icon::SearchX,
    title(ctx, tab, filters),
    ctx.t("memory.noMatch.lexicalShort"),
  )
  .page();
  let other_types = listing::found_elsewhere(records, filters);
  if other_types > 0 {
    let note = match other_types {
      1 => ctx.t("memory.noMatch.otherTypesOne"),
      count => ctx.t_args("memory.noMatch.otherTypes", [("count", count.to_string())]),
    };
    let shown = state.filters.clone();
    empty = empty.note(note).action(
      ActionButton::new(
        "memory-show-all-types",
        ButtonKind::Secondary,
        ctx.t("memory.noMatch.showAllTypes"),
      )
      .icon(Icon::List)
      .on_click(move || shown.update(|filters| filters.kind = None)),
    );
  } else {
    let elsewhere = elsewhere(memory, tab, order, filters);
    if let Some(note) = matches_line(ctx, &elsewhere) {
      empty = empty.note(note);
    }
    let navigation = Navigation::of(ctx);
    for (other, _) in &elsewhere {
      let open = navigation.to(tabs::route(state.project_id, *other));
      empty = empty.action(
        ActionButton::new(
          format!("memory-open-tab-{}", other.name()),
          ButtonKind::Secondary,
          ctx.t_args("memory.tabs.open", [("tab", &*tabs::name(ctx, *other))]),
        )
        .icon(tabs::glyph(*other))
        .opens(open),
      );
    }
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

/// *No decisions match “retry”*, *No active records match “retry”*, or *No records match these filters* without a
/// search.
fn title(ctx: &Ctx, tab: MemoryTab, filters: &Filters) -> Arc<str> {
  let query = filters.search.trim();
  match (query.is_empty(), filters.kind) {
    (false, Some(kind)) => ctx.t_args(
      "memory.noMatch.kindSearch",
      [
        ("kind", words::kind_plural(ctx, kind).to_string()),
        ("query", query.to_owned()),
      ],
    ),
    (false, None) => {
      let key = match tab {
        MemoryTab::Waiting => "memory.noMatch.tabSearch.waiting",
        MemoryTab::Active => "memory.noMatch.tabSearch.active",
        MemoryTab::Deprecated => "memory.noMatch.tabSearch.deprecated",
        MemoryTab::Superseded => "memory.noMatch.tabSearch.superseded",
        MemoryTab::Rejected | MemoryTab::Settings => "memory.noMatch.tabSearch.rejected",
      };
      ctx.t_args(key, [("query", query)])
    }
    (true, _) => ctx.t("memory.noMatch.filters"),
  }
}

/// The other status tabs, in their order, with how many records `filters` show in each; the tabs with none are left
/// out.
fn elsewhere(memory: &ProjectMemory, tab: MemoryTab, order: tabs::Order, filters: &Filters) -> Vec<(MemoryTab, usize)> {
  order
    .statuses()
    .into_iter()
    .filter(|other| *other != tab)
    .filter_map(|other| {
      let status = tabs::statuses(other)?;
      let in_other = Filters {
        status,
        ..filters.clone()
      };
      let count = listing::shown(memory.pool(status), &in_other);
      (count > 0).then_some((other, count))
    })
    .collect()
}

/// *1 match in Deprecated*, *3 matches in Rejected*, or *1 in Deprecated · 2 in Rejected*.
fn matches_line(ctx: &Ctx, elsewhere: &[(MemoryTab, usize)]) -> Option<Arc<str>> {
  match elsewhere {
    [] => None,
    [(tab, count)] => {
      let key = if *count == 1 {
        "memory.tabs.matchInOne"
      } else {
        "memory.tabs.matchesIn"
      };
      Some(ctx.t_args(
        key,
        [("count", count.to_string()), ("tab", tabs::name(ctx, *tab).to_string())],
      ))
    }
    several => {
      let parts: Vec<String> = several
        .iter()
        .map(|(tab, count)| {
          ctx
            .t_args(
              "memory.tabs.inTab",
              [("count", count.to_string()), ("tab", tabs::name(ctx, *tab).to_string())],
            )
            .to_string()
        })
        .collect();
      Some(words::join(ctx, &parts))
    }
  }
}
