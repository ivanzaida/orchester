//! What a record list says when its filters show nothing (`n3lxin` in `fQ33n`, `z4t51u`): that nothing matches,
//! how search works, what matches among other types or statuses with a way to show them, and *Clear search*. The
//! project's page names the status it lists (*No active records match “oauth refresh”*) and, when the other matches
//! share one status, that status with a way to show it (*1 match among deprecated records*, *Show deprecated*).

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
  let page = state.task.is_none();
  let lexical = ctx.t(if page {
    "memory.noMatch.lexicalShort"
  } else {
    "memory.noMatch.lexical"
  });
  let elsewhere = elsewhere(ctx, state, records, filters);
  let explanation = match &elsewhere.explanation {
    Some(more) => format!("{lexical} {more}"),
    None => lexical.to_string(),
  };
  let mut empty = Centered::new("memory-no-match", Icon::SearchX, title(ctx, page, filters), explanation).page(page);
  if let Some(note) = elsewhere.note {
    empty = empty.note(note);
  }
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

/// *No decisions match “retry”*, *No active records match “retry”* on the project's page, *No records match “retry”*,
/// or *No records match these filters* without a search.
fn title(ctx: &Ctx, page: bool, filters: &Filters) -> Arc<str> {
  let query = filters.search.trim();
  match (query.is_empty(), filters.kind) {
    (false, Some(kind)) => ctx.t_args(
      "memory.noMatch.kindSearch",
      [
        ("kind", words::kind_plural(ctx, kind).to_string()),
        ("query", query.to_owned()),
      ],
    ),
    (false, None) if page && filters.status.single() => {
      let status = words::status_filter(ctx, filters.status).to_lowercase();
      ctx.t_args(
        "memory.noMatch.statusSearch",
        [("status", status.as_str()), ("query", query)],
      )
    }
    (false, None) => ctx.t_args("memory.noMatch.search", [("query", query)]),
    (true, _) => ctx.t("memory.noMatch.filters"),
  }
}

/// What matches outside the filters: words after the explanation or a note under it, and a way to show it.
#[derive(Default)]
struct Elsewhere {
  explanation: Option<String>,
  note: Option<Arc<str>>,
  show: Option<ActionButton>,
}

fn elsewhere(ctx: &Ctx, state: &MemoryState, records: &[Memory], filters: &Filters) -> Elsewhere {
  let other_types = listing::found_elsewhere(records, filters);
  let (other_statuses, shared) = listing::found_in_other_statuses(records, filters);
  let show = |id: &str, label: Arc<str>, change: Box<dyn Fn(&mut Filters) + Send + Sync>| {
    let filters = state.filters.clone();
    ActionButton::new(id, ButtonKind::Secondary, label)
      .icon(Icon::List)
      .on_click(move || filters.update(|filters| change(filters)))
  };
  if let Some(status) = shared.filter(|_| state.task.is_none() && other_types == 0) {
    let named = words::status_filter(ctx, status).to_lowercase();
    let note = if other_statuses == 1 {
      ctx.t_args("memory.noMatch.amongOne", [("status", named.as_str())])
    } else {
      ctx.t_args(
        "memory.noMatch.among",
        [("count", other_statuses.to_string()), ("status", named.clone())],
      )
    };
    let label = ctx.t_args("memory.noMatch.showStatus", [("status", named.as_str())]);
    return Elsewhere {
      explanation: None,
      note: Some(note),
      show: Some(show(
        "memory-show-status",
        label,
        Box::new(move |filters| filters.status = status),
      )),
    };
  }
  if other_types > 0 {
    let explanation = match other_types {
      1 => ctx.t("memory.noMatch.otherTypesOne"),
      count => ctx.t_args("memory.noMatch.otherTypes", [("count", count.to_string())]),
    };
    return Elsewhere {
      explanation: Some(explanation.to_string()),
      note: None,
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
      note: None,
      show: Some(show(
        "memory-show-all-statuses",
        ctx.t("memory.noMatch.showAllStatuses"),
        Box::new(|filters| filters.status = StatusFilter::All),
      )),
    };
  }
  Elsewhere::default()
}
