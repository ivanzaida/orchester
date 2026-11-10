//! The parts a local project's panels share (`Detail` in L1, L2): the frame (the body scrolling over the action pinned
//! at its bottom), the chips under a head (how the entry stands, who uses it) and *Ready?*, what the entry's readiness
//! lacks, the head's end of a resource with versions, *Used by* of an instruction, a skill or a tool, and a panel in
//! bones while what it edits is read for the first time. An MCP server's panel is [`super::mcp`], an archived entry's
//! [`super::archived`], a profile's [`super::profile`], an instruction's [`super::instruction`], a skill's
//! [`super::skill`] and a tool's [`super::tool`] ([`super::editing`] for one that is no command).

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
use orchester_core::id::ResourceId;
use orchester_workspace::{
  local_archive::Archivable,
  readiness::{Group, ReadinessStanding},
};

use super::{
  archive,
  model::Entry,
  save_bar::{self, Save},
  words,
};
use crate::{
  project::ShownProject,
  screens::{
    hub::versions::{VersionsProps, versions_button},
    team_hub::{
      panel::{body, head_named},
      rows::{self, Sub},
    },
  },
  theme::{HubSpacing, HubText, IconSize},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
    skeleton,
  },
};

/// How long a value of words is in bones, and the bone in a field's frame.
const BONE_WORDS: f32 = 140.0;
const BONE_VALUE: f32 = 96.0;

/// What a row's value is in bones: words, a one-line field or choice, or a text area at least this high.
#[derive(Clone, Copy)]
pub(super) enum Bone {
  Words,
  Field,
  Area(f32),
}

/// A row in bones: its label and the words under it real (keys), its value a bone.
pub(super) struct BoneRow {
  pub label: &'static str,
  pub sub: Option<&'static str>,
  pub bone: Bone,
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

/// How the entry stands, as the chips after its version: only a draft so far, what it needs, or ready.
pub(super) fn standing_chips(ctx: &Ctx, entry: &Entry) -> Vec<(Arc<str>, BadgeTone)> {
  let mut chips = Vec::new();
  if let Some(number) = entry.number {
    chips.push((Arc::from(number.to_string()), BadgeTone::Neutral));
  }
  chips.push(match (entry.number, entry.group, entry.standing.as_ref()) {
    (None, ..) => (ctx.t("local_hub.chip.draft"), BadgeTone::Info),
    (Some(_), Group::NeedsYouHere, Some(standing)) => (words::needs(ctx, entry, standing), BadgeTone::Warning),
    _ => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
  });
  chips
}

/// *Versions* and the … menu at the end of the head of the resource `id`, 8 apart; *Versions*' ids start with
/// `prefix`.
pub(super) fn head_end(
  ctx: &mut Ctx,
  project: &ShownProject,
  entry: &Entry,
  (id, prefix): (ResourceId, &str),
) -> Element {
  let versions = versions_button(
    ctx,
    VersionsProps {
      id: prefix.into(),
      root: project.open.repo_root.clone(),
      resource: (entry.kind, id),
      local: true,
    },
  );
  let more = archive::menu(
    ctx,
    archive::Target {
      root: project.open.repo_root.clone(),
      project: project.open.project.id(),
      what: entry.what,
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(versions)
    .child(more)
    .into()
}

/// What a panel in bones shows as it is: the head with its end, the chips and the bar, which the hub knows before what
/// the panel edits is read.
pub(super) struct Known {
  pub head: Row,
  pub chips: Row,
  pub bar: Element,
}

/// The panel of the resource `id` (`entry`) while what it edits is read for the first time: the head, its end and the
/// chips the hub knows already, then the section `title` (a key) with `rows` in bones, and the bar with *Save* and
/// *Cancel* off, so that the editor takes their place without the panel moving. The ids start with `prefix`.
pub(super) fn bones(
  ctx: &mut Ctx,
  (project, entry, id): (&ShownProject, &Entry, ResourceId),
  prefix: &str,
  section: (&str, &[BoneRow]),
) -> Element {
  let head = head_named(
    &entry.name,
    (
      ctx.t(words::kind_where_key(entry.kind)).to_string(),
      entry.number.map(|number| number.to_string()),
    ),
    Some(head_end(ctx, project, entry, (id, prefix))),
  );
  let known = Known {
    head,
    chips: chips_of(standing_chips(ctx, entry), None),
    bar: save_bar::bar(ctx, prefix, (Save::Unchanged, None), save_bar::makes(ctx, entry.number)),
  };
  bones_with(ctx, entry.what, prefix, section, known)
}

/// The panel of `what` in bones: what is `known` as it is, the section `title` (a key) with `rows` in bones between the
/// chips and the bar. The ids start with `prefix`.
pub(super) fn bones_with(
  ctx: &Ctx,
  what: Archivable,
  prefix: &str,
  (title, rows): (&str, &[BoneRow]),
  known: Known,
) -> Element {
  let rows = rows
    .iter()
    .enumerate()
    .map(|(index, row)| bone_row(ctx, &format!("{prefix}-bone-{index}"), row))
    .collect();
  let section = rows::section(&format!("{prefix}-bones"), &ctx.t(title), Vec::new(), rows);
  let body = body()
    .child(known.head)
    .child(known.chips)
    .child(skeleton::pulsing(section));
  frame(what, body, Some(known.bar))
}

fn bone_row(ctx: &Ctx, id: &str, row: &BoneRow) -> Row {
  let value: Element = match row.bone {
    Bone::Words => skeleton::line(ctx, HubText::Value.role(), BONE_WORDS).into(),
    Bone::Field => skeleton::field(ctx, BONE_WORDS, BONE_VALUE)
      .width(Dimension::Pct(100.0))
      .into(),
    Bone::Area(height) => skeleton::bone(BONE_WORDS, height).width(Dimension::Pct(100.0)).into(),
  };
  let row_of = rows::row(id, &ctx.t(row.label), row.sub.map(|sub| Sub::quiet(ctx.t(sub))), value);
  match row.bone {
    Bone::Area(_) => row_of.align_items(Alignment::Start),
    Bone::Words | Bone::Field => row_of,
  }
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

/// *Used by*: the profiles that give their agents the instruction, skill or tool `entry`, read-only. Nothing else gives
/// an agent one (`docs/design/team-configuration.md`, "Decided 2026-11-09"), so with none no agent gets it, which the
/// row says.
pub(super) fn used_by(ctx: &Ctx, prefix: &str, entry: &Entry) -> Row {
  let id = format!("{prefix}-used-by");
  let value: Element = if entry.used_by.is_empty() {
    Text::new(&ctx.t("local_hub.given.none"))
      .id(format!("{id}-value"))
      .variant(HubText::Value.role())
      .color(PaletteColor::TextMuted)
      .into()
  } else {
    rows::tags(&format!("{id}-value"), entry.used_by.iter().cloned()).into()
  };
  rows::row(
    &id,
    &ctx.t("local_hub.given.usedBy"),
    Some(Sub::quiet(ctx.t("local_hub.given.usedBySub"))),
    value,
  )
}
