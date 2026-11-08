//! The panel beside the list in a local project (`Detail` in L1, L2): the chosen entry's head (what it is, its version
//! when it has one, its name), its chips (how it stands, who uses it), what its readiness lacks, and the action that
//! moves it on pinned at its bottom. An MCP server's panel is [`super::mcp`], an archived entry's [`super::archived`];
//! the others (profile, instruction, skill, tool) are this frame with a summary, and *Edit* opens the editor the kind
//! has, which edits them in place with their drafts and versions.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, resource::ResourceKind};
use orchester_workspace::{
  local_archive::Archivable,
  readiness::{Group, ReadinessStanding},
};

use super::{
  archive,
  model::Entry,
  words::{self, kind_where_key},
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{AgentTab, InstructionsTab, InstructionsView, ProjectPage, Route, ToolTab},
  screens::team_hub::{
    bar::bar,
    panel::{body, head_named},
  },
  theme::{HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// What a panel needs besides the entry.
pub(super) struct Shown<'a> {
  pub project: ProjectId,
  pub root: &'a std::path::Path,
  pub navigation: &'a Navigation,
}

/// The panel of a profile, tool, instruction or skill: its summary, what it lacks, and *Edit*.
pub(super) fn panel(ctx: &mut Ctx, entry: &Entry, shown: &Shown<'_>) -> Element {
  let more = archive::menu(
    ctx,
    archive::Props {
      root: shown.root.to_path_buf(),
      project: shown.project,
      what: entry.what,
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  let version = entry.number.map(|number| number.to_string());
  let body = body()
    .child(head_named(
      &entry.name,
      (ctx.t(kind_where_key(entry.kind)).to_string(), version),
      Some(more),
    ))
    .child(chips(ctx, entry))
    .with_children(summary(ctx, entry))
    .with_children(readiness(ctx, entry));
  let edit = ActionButton::new("local-hub-edit", ButtonKind::Primary, ctx.t("local_hub.panel.edit"))
    .icon(Icon::Pencil)
    .pinned()
    .opens(open_editor(entry, shown));
  frame(
    entry.what,
    body,
    Some(bar(
      vec![edit.into()],
      Some(ctx.t("local_hub.panel.editHint").to_string()),
    )),
  )
}

/// The editor the kind has, which edits the resource in place.
fn open_editor(entry: &Entry, shown: &Shown<'_>) -> Open {
  let Archivable::Resource(id) = entry.what else {
    unreachable!("an MCP server has its own panel");
  };
  let page = match entry.kind {
    ResourceKind::AgentProfile => ProjectPage::Agent(id, AgentTab::Profile),
    ResourceKind::Tool => ProjectPage::Tool(id, ToolTab::default()),
    ResourceKind::Instruction => ProjectPage::Instructions(InstructionsView::rule(InstructionsTab::Project, Some(id))),
    _ => ProjectPage::Skill(id),
  };
  shown.navigation.to(Route::Project(shown.project, page))
}

/// The panel of `what`: `body` scrolls over the pinned `bar`.
pub(super) fn frame(what: Archivable, body: Column, bar: Option<Element>) -> Element {
  let scroll = ScrollVertical::new(body)
    .id("local-hub-panel-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("local-hub-panel")
    .describe("entry", super::row::token(what))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(bar)
    .into()
}

/// The chips under the head: how the entry stands, and at their end who uses it.
pub(super) fn chips(ctx: &Ctx, entry: &Entry) -> Row {
  let (stand, tone) = standing_chip(ctx, entry);
  let end = used_by(ctx, entry);
  chips_of(vec![(stand, tone)], end)
}

/// The chips `badges`, then `end` (*used by Designer, Reviewer*) at their end; it ends in an ellipsis when it runs out
/// of room.
pub(super) fn chips_of(badges: Vec<(Arc<str>, BadgeTone)>, end: Option<Arc<str>>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      badges
        .into_iter()
        .enumerate()
        .map(|(index, (words, tone))| badge(&format!("local-hub-chip-{index}"), tone, &words)),
    )
    .child(Row::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id("local-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

fn standing_chip(ctx: &Ctx, entry: &Entry) -> (Arc<str>, BadgeTone) {
  match entry.group {
    Group::NeedsYouHere => match entry
      .standing
      .as_ref()
      .map(|standing| words::needs(ctx, entry, standing))
    {
      Some(needs) => (needs, BadgeTone::Warning),
      None => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
    },
    Group::Ready => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
    Group::OffHere => (ctx.t("local_hub.chip.off"), BadgeTone::Neutral),
  }
}

/// *used by Feature delivery, Quick fix*: who uses the entry; nothing when nothing does.
fn used_by(ctx: &Ctx, entry: &Entry) -> Option<Arc<str>> {
  let names: Vec<String> = match &entry.profile {
    Some(profile) => profile.pipelines.clone(),
    None => entry.used_by.clone(),
  };
  (!names.is_empty()).then(|| ctx.t_args("local_hub.panel.usedBy", [("names", names.join(", "))]))
}

/// A profile's values in the panel: the connection it runs on and the MCP servers it chooses.
fn summary(ctx: &Ctx, entry: &Entry) -> Option<Column> {
  let profile = entry.profile.as_ref()?;
  let mut rows = Vec::new();
  if let Some(connection) = &profile.connection {
    rows.push((ctx.t("local_hub.panel.connection"), connection.clone()));
  }
  if !profile.mcp_servers.is_empty() {
    let names: Vec<&str> = profile.mcp_servers.iter().map(|(_, name)| name.as_str()).collect();
    rows.push((ctx.t("local_hub.panel.mcpServers"), names.join(", ")));
  }
  (!rows.is_empty()).then(|| {
    Column::new()
      .id("local-hub-summary")
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Md)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .with_children(rows.into_iter().enumerate().map(|(index, (label, value))| {
        Row::new()
          .id(format!("local-hub-summary-{index}"))
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
          .spacing(SpacingSize::Md)
          .child(
            Text::new(&label)
              .variant(HubText::PanelLabel.role())
              .width(Dimension::Px(160.0)),
          )
          .child(Text::new(&value).variant(HubText::Value.role()).flex(1.0))
      }))
  })
}

/// *Ready?*: the parts that are missing, one row each; none for an entry that lacks nothing.
pub(super) fn readiness(ctx: &Ctx, entry: &Entry) -> Option<Column> {
  let Some(ReadinessStanding::Missing(missing)) = &entry.standing else {
    return None;
  };
  if entry.group == Group::OffHere {
    return None;
  }
  let rows = missing.missing().iter().enumerate().map(|(index, part)| {
    Row::new()
      .id(format!("local-hub-missing-{index}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&words::missing(ctx, entry, part))
          .variant(HubText::Value.role())
          .flex(1.0),
      )
  });
  Some(
    Column::new()
      .id("local-hub-readiness")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("local_hub.readiness.title")).variant(HubText::PanelLabel.role()))
          .child(
            Text::new(&ctx.t_args(
              "local_hub.readiness.missing",
              [("count", missing.missing().len().to_string())],
            ))
            .variant(HubText::PanelHint.role())
            .color(PaletteColor::Warning),
          ),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .rounded(RadiusSize::Md)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .with_children(rows),
      ),
  )
}
