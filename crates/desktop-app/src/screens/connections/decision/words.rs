//! Frame 07's status sentences, from recorded checks rather than transport error text.
use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{DecisionCheckResult, DecisionConnection};
use orchester_workspace::display;

use crate::{
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::icon::{Icon, icon},
};

/// The status dot in 02 (`Y9HsVU`).
const DOT: f32 = 6.0;

pub(super) fn line(ctx: &Ctx, connection: &DecisionConnection, pending: bool) -> Element {
  let (words, color) = sentence(ctx, connection, pending);
  let mark: Element = if pending {
    icon(Icon::LoaderCircle, IconSize::Sm, PaletteColor::TextSecondary).into()
  } else {
    Row::new()
      .width(Dimension::Px(DOT))
      .height(Dimension::Px(DOT))
      .rounded(crate::theme::ExtraRadius::Full.role())
      .background(color)
      .into()
  };
  let words = if let Some(check) = connection.last_check().filter(|_| !pending) {
    ctx.t_args(
      "decision_connection.checked",
      [("status", words.to_string()), ("time", display::clock(check.at))],
    )
  } else {
    words
  };
  Row::new()
    .align_items(Alignment::Center)
    .spacing(ExtraSpacing::Snug.role())
    .child(mark)
    .child(
      Text::new(&words)
        .id("decision-status")
        .variant(ProjectSettingsText::Hint.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0),
    )
    .into()
}

fn sentence(ctx: &Ctx, connection: &DecisionConnection, pending: bool) -> (Arc<str>, PaletteColor) {
  let rest = connection
    .base_url()
    .as_str()
    .split_once("://")
    .map_or("", |(_, rest)| rest);
  let (host, path) = rest.find('/').map_or((rest, ""), |at| rest.split_at(at));
  if pending {
    return (
      ctx.t_args("decision_connection.checking", [("host", host)]),
      PaletteColor::TextMuted,
    );
  }
  match connection.last_check().map(|check| &check.result) {
    Some(DecisionCheckResult::Connected) => (ctx.t("decision_connection.connected"), PaletteColor::Success),
    Some(DecisionCheckResult::KeyRefused) => (ctx.t("decision_connection.keyRefused"), PaletteColor::Danger),
    Some(DecisionCheckResult::NothingAnswers) => (
      ctx.t_args("decision_connection.nothingAnswers", [("host", host)]),
      PaletteColor::Danger,
    ),
    Some(DecisionCheckResult::NotSystemOneApi) => (
      ctx.t_args("decision_connection.wrongApi", [("host", host), ("path", path)]),
      PaletteColor::Warning,
    ),
    Some(DecisionCheckResult::ModelGone { model }) => (
      ctx.t_args("decision_connection.modelGone", [("model", model.as_str())]),
      PaletteColor::Warning,
    ),
    None => (ctx.t("decision_connection.notChecked"), PaletteColor::TextMuted),
  }
}

pub(super) fn note_key(connection: &DecisionConnection) -> Option<&'static str> {
  match connection.last_check().map(|check| &check.result) {
    Some(DecisionCheckResult::KeyRefused) => Some("decision_connection.keyNote"),
    Some(DecisionCheckResult::NothingAnswers) if connection.is_on_this_computer() => {
      Some("decision_connection.localNote")
    }
    Some(DecisionCheckResult::NothingAnswers) => Some("decision_connection.remoteNote"),
    Some(DecisionCheckResult::NotSystemOneApi) => Some("decision_connection.apiNote"),
    Some(DecisionCheckResult::ModelGone { .. }) => Some("decision_connection.modelNote"),
    _ => None,
  }
}
