//! A row of the hub's list (`Item · Builder` `tF2OL`, `Item · pencil-design` `U9RFS`): the kind's glyph, the name over
//! what this computer needs, and at its end a tag and a version. Under All the tag names the kind; under a kind with
//! something to fill in on a computer (profiles, MCP servers, tools) the tag names what an MCP server or a tool is
//! and the version stands after it. The chosen row stands inert on `hover` with a 2 `accent` start (`Item ·
//! Builder` `n330IO`); the others open the hub on them.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  node::Element,
};
use orchester_workspace::readiness::Group;

use super::{
  list::Shown,
  model::{Entry, Kind, Listing, form_key},
  words,
};
use crate::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  screens::hub::agent_row::{AgentRow, agent_row, line, tag_chip},
  theme::IconSize,
  ui::{agent_mark::AgentMark, icon::icon, pressable::opening},
};

pub(super) fn row(ctx: &Ctx, listing: &Listing, entry: &Entry, shown: &Shown<'_>) -> Element {
  let id = format!("team-hub-row-{}", entry.id);
  let route = Route::Project(
    shown.project,
    ProjectPage::hub(HubView::new(shown.filter, Some(entry.id))),
  );
  let said = words::line(ctx, listing, entry);
  agent_row(AgentRow {
    id: &id,
    chosen: shown.selected.is_some_and(|selected| selected.id == entry.id),
    open: opening(shown.navigation.to(route)),
    group: Some(entry.group),
    kind: entry.kind.name().to_owned(),
    mark: mark(ctx, entry),
    name: (&entry.name, PaletteColor::TextPrimary),
    line: line(&id, &said.text, said.color).into(),
    tag: tag(ctx, entry, shown.filter, &id),
    version: version(entry, shown.filter),
  })
}

/// The glyph at a row's start: a profile's CLI's own mark, else the glyph of what the resource is.
fn mark(ctx: &Ctx, entry: &Entry) -> Element {
  match entry.cli {
    Some(cli) => AgentMark::of(cli).render(ctx, IconSize::Lg),
    None => icon(entry.glyph(), IconSize::Lg, glyph_color(entry)).into(),
  }
}

/// What a row's glyph says: how the resource stands here, in the color of its line.
fn glyph_color(entry: &Entry) -> PaletteColor {
  match (entry.group, entry.kind) {
    (Group::NeedsYouHere, _) if words::said_in_danger(entry) => PaletteColor::Danger,
    (Group::NeedsYouHere, _) => PaletteColor::Warning,
    (Group::OffHere, _) => PaletteColor::TextMuted,
    (Group::Ready, Kind::Instruction | Kind::Skill | Kind::Profile) => PaletteColor::TextSecondary,
    (Group::Ready, Kind::McpServer | Kind::Tool) => PaletteColor::Success,
  }
}

/// Under All the kind's tag; under a kind with something to fill in, what the MCP server or the tool is.
fn tag(ctx: &Ctx, entry: &Entry, filter: HubFilter, id: &str) -> Option<Row> {
  let key = if filter == HubFilter::All {
    entry.kind.tag_key()
  } else {
    form_key(entry.form?)
  };
  Some(tag_chip(&format!("{id}-tag"), &ctx.t(key)))
}

/// The version a computer uses at a row's end (`Meta` in `U9RFS`), under a kind with something to fill in.
fn version(entry: &Entry, filter: HubFilter) -> Option<String> {
  if filter == HubFilter::All || !entry.kind.fills_in() {
    return None;
  }
  entry.number.map(|number| number.to_string())
}
