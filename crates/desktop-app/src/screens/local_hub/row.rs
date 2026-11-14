//! A row of the hub's list in a local project (`Item · pencil-design` in L2): the kind's glyph, the name over what the
//! person has to do, and at its end a tag. A local MCP server has no version, so no row of it says one. The chosen row
//! stands inert on `hover` with a 2 `accent` start; the others open the hub on them.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};
use orchester_core::resource::ResourceKind;
use orchester_workspace::{local_archive::Archivable, readiness::Group};

use super::{
  list::Shown,
  model::{Entry, view_of},
  words::{self, Unsaved},
};
use crate::{
  routes::{HubFilter, ProjectPage, Route},
  screens::hub::agent_row::{AgentRow, agent_row, line, tag_chip},
  theme::IconSize,
  ui::{agent_mark::AgentMark, icon::icon},
};

/// The element id of an entry's parts.
pub(super) fn token(what: Archivable) -> String {
  match what {
    Archivable::Resource(id) => id.to_string(),
    Archivable::McpServer(id) => format!("mcp-{id}"),
  }
}

pub(super) fn row(ctx: &Ctx, entry: &Entry, shown: &Shown<'_>) -> Element {
  let id = format!("local-hub-row-{}", token(entry.what));
  let route = Route::Project(shown.project, ProjectPage::hub(view_of(shown.filter, Some(entry.what))));
  let unsaved = shown
    .unsaved
    .and_then(|(what, state)| (what == entry.what).then_some(state));
  let said = words::line(ctx, (shown.now, shown.whose), entry, unsaved);
  agent_row(AgentRow {
    id: &id,
    chosen: shown.selected == Some(entry.what),
    // Opening another entry would drop unsaved edits: the hub asks first.
    open: shown.leave.press(route).button(),
    group: Some(entry.group),
    kind: format!("{:?}", entry.kind),
    mark: mark(ctx, entry, unsaved),
    name: (&entry.name, PaletteColor::TextPrimary),
    line: line(&id, &said.text, said.color).into(),
    tag: tag(ctx, entry, shown.filter, &id),
    version: version(entry, shown.filter),
  })
}

/// The glyph at a row's start: a profile's CLI's own mark, else the glyph of what it is, in the color of how it stands.
fn mark(ctx: &Ctx, entry: &Entry, unsaved: Option<Unsaved>) -> Element {
  if entry.archived.is_some() {
    return icon(entry.glyph(), IconSize::Lg, PaletteColor::TextMuted).into();
  }
  if let Some(cli) = entry.cli {
    return AgentMark::of(cli).render(ctx, IconSize::Lg);
  }
  let color = match (unsaved, entry.group) {
    (Some(Unsaved::Refused), _) => PaletteColor::Danger,
    (Some(Unsaved::Edited), _) | (None, Group::NeedsYouHere) => PaletteColor::Warning,
    (None, Group::OffHere) => PaletteColor::TextMuted,
    (None, Group::Ready) => match entry.kind {
      ResourceKind::McpServer | ResourceKind::Tool => PaletteColor::Success,
      _ => PaletteColor::TextSecondary,
    },
  };
  icon(entry.glyph(), IconSize::Lg, color).into()
}

/// Under All and Archived the kind's tag; under a kind what an MCP server or a tool is.
fn tag(ctx: &Ctx, entry: &Entry, filter: HubFilter, id: &str) -> Option<Row> {
  let key = if matches!(filter, HubFilter::All | HubFilter::Archived) {
    words::kind_tag_key(entry.kind)?
  } else {
    words::form_key(entry.form?)
  };
  Some(tag_chip(&format!("{id}-tag"), &ctx.t(key)))
}

/// The version at a row's end (`Meta` in L1), under a kind's own filter: the latest published; an MCP server has none.
fn version(entry: &Entry, filter: HubFilter) -> Option<String> {
  if matches!(filter, HubFilter::All | HubFilter::Archived) {
    return None;
  }
  entry.number.map(|number| number.to_string())
}
