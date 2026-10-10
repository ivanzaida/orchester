//! Draft card LruUv and shared framing for the enforcement card VsylR.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::agents::data::VersionState;

use super::{editor::Props, fields::Fields, words};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    card::{CardSurface, card_frame},
    icon::{Icon, icon},
  },
};
/// LruUv and VsylR.
const PANEL_WIDTH: f32 = 320.0;

pub fn panel(id: &str) -> Column {
  card_frame(id, CardSurface::Panel).width(Dimension::Px(PANEL_WIDTH))
}
pub fn heading(glyph: Icon, title: &str, meta: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(Text::new(title).variant(TypographyStyle::Label).flex(1.0))
    .child(Text::new(meta).variant(TypographyStyle::Caption))
}
pub fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Md)
}
pub fn draft(ctx: &Ctx, props: &Props, fields: &Fields) -> Element {
  let state = props
    .entry
    .as_ref()
    .map_or(VersionState::Draft { published: None }, |e| e.state);
  let version = state.published().map_or(0, |v| v.get());
  let dirty = props
    .entry
    .as_ref()
    .is_none_or(|e| fields.profile(&props.listed).as_ref() != Ok(&e.profile));
  if !state.has_draft() && !dirty {
    return panel("agent-draft-card")
      .child(heading(Icon::CircleCheck, &words::version(ctx, state, false), ""))
      .child(body().child(words::note(&words::state_note(ctx, state, false))))
      .into();
  }
  let saved = props.entry.as_ref().and_then(|e| props.listed.draft_times.get(&e.id));
  let meta = if dirty {
    ctx.t("agents.unsavedNew")
  } else {
    saved.map_or_else(
      || ctx.t("agents.notPublished"),
      |at| ctx.t_args("agents.savedAt", [("time", orchester_workspace::display::clock(*at))]),
    )
  };
  let mut content = body();
  let base = match state {
    VersionState::StaleDraft { based_on, .. } => based_on,
    _ => state.published(),
  };
  if let Some(base) = base {
    content = content.child(words::note(
      &ctx.t_args("agents.basedOn", [("version", base.get().to_string())]),
    ));
  }
  if let Some(entry) = props
    .entry
    .as_ref()
    .and_then(|current| props.listed.published.iter().find(|entry| entry.id == current.id))
  {
    content = content.child(
      Text::new(&ctx.t_args("agents.changes", [("version", version.to_string())])).variant(TypographyStyle::Caption),
    );
    for (key, detail) in changes(ctx, fields, &entry.profile) {
      content = content.child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(words::note(&ctx.t(key)).flex(1.0))
          .child(words::note(&detail).color(PaletteColor::TextPrimary)),
      );
    }
  }
  content = content.child(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Md))
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(words::note(
        &ctx.t_args("agents.publishNote", [("version", (version + 1).to_string())]),
      )),
  );
  panel("agent-draft-card")
    .child(heading(
      Icon::FilePen,
      &ctx.t_args("agents.draftCard", [("version", (version + 1).to_string())]),
      &meta,
    ))
    .child(content)
    .into()
}
fn changes(
  ctx: &Ctx,
  fields: &Fields,
  base: &orchester_core::resource::AgentProfile,
) -> Vec<(&'static str, std::sync::Arc<str>)> {
  let changed = ctx.t("agents.changed");
  let mut rows = Vec::new();
  for (different, key) in [
    (fields.role.get() != base.role().as_str(), "agents.role"),
    (
      fields.responsibility.get() != base.responsibilities(),
      "agents.responsibility",
    ),
    (
      fields.instructions.get() != base.instructions().as_str(),
      "agents.instructions",
    ),
    (
      fields.permissions.get().0 != *base.permissions(),
      "agents.permissionsTab",
    ),
    (fields.servers.get().0 != *base.mcp_servers(), "agents.mcp"),
    (
      fields.connection.get() != base.connection().to_string(),
      "agents.connection",
    ),
  ] {
    if different {
      rows.push((key, changed.clone()));
    }
  }
  for (key, from, to) in [
    ("agents.model", base.model().as_str().to_owned(), fields.model.get()),
    (
      "agents.effort",
      orchester_workspace::agents::form::stored_effort(base).unwrap_or_default(),
      fields.effort.get(),
    ),
  ] {
    if from != to {
      let from = if from.is_empty() {
        ctx.t("agents.defaultEffort")
      } else {
        from.into()
      };
      let to = if to.is_empty() {
        ctx.t("agents.defaultEffort")
      } else {
        to.into()
      };
      rows.push((key, ctx.t_args("agents.changedValue", [("from", from), ("to", to)])));
    }
  }
  rows
}
