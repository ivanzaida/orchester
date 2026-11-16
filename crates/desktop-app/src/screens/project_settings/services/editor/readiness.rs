//! Readiness (`Readiness` `kK9S6`): *Ready when* with its three checks as radios, the HTTP path in a 180 field beside
//! its radio and the log pattern in a 200 one beside its (`TxJPu`, `hNg9Q`), then *Timeout* in a 120 field with
//! *seconds* after the number (`l7SsW`). A check chosen, and a field left, saves; the path and the pattern each stay
//! as typed while another check is chosen.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::agent_testing::editor::{form::ReadyKind, order::Field};

use super::{
  super::super::kit::Group,
  Commit, View,
  fields::{field, input, placeholder_in, with_error},
  sections::{ReadyChoice, Sections},
};
use crate::{
  theme::{ControlHeight, ExtraSpacing},
  ui::{
    field::FieldInput,
    toggle::{Toggle, ToggleKind},
  },
};

/// The HTTP path's field (`Input` `ZzQKS`).
const PATH_WIDTH: f32 = 180.0;
/// The log pattern's field (`Input` `dvyLD`).
const PATTERN_WIDTH: f32 = 200.0;
/// The timeout's field (`Input` `l7SsW`).
const TIMEOUT_WIDTH: f32 = 120.0;

/// The Readiness group.
pub(super) fn readiness_group(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let checks = checks(ctx, sections, view, commit);
  let ready = field(
    "settings-service-ready",
    ctx.t("project_settings.services.editor.readiness.when"),
    ctx.t("project_settings.services.editor.readiness.whenHint"),
  )
  .render(ctx, checks);
  let timeout = timeout_row(ctx, sections, view, commit);
  Group::new(
    "settings-service-readiness-group",
    ctx.t("project_settings.services.editor.readiness.title"),
  )
  .row(ready)
  .row(timeout)
  .into()
}

/// The three checks, each a radio; the chosen one's error under its field.
fn checks(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let chosen = sections.ready.get().0;
  let error = view
    .problems()
    .and_then(|problems| problems.readiness.clone())
    .or_else(|| placeholder_in(view, Field::Readiness));
  let radio = |kind: ReadyKind, (id, key): (&str, &str), ctx: &Ctx| {
    let (ready, commit) = (sections.ready.clone(), commit.clone());
    Element::from(
      Toggle::new(id.to_owned(), ToggleKind::Radio, chosen == kind, ctx.t(key)).on_change(move |_| {
        ready.set(ReadyChoice(kind));
        commit();
      }),
    )
  };
  let port = radio(
    ReadyKind::Port,
    (
      "settings-service-ready-port",
      "project_settings.services.editor.readiness.port",
    ),
    ctx,
  );
  let http = radio(
    ReadyKind::Http,
    (
      "settings-service-ready-http",
      "project_settings.services.editor.readiness.http",
    ),
    ctx,
  );
  let log = radio(
    ReadyKind::Log,
    (
      "settings-service-ready-log",
      "project_settings.services.editor.readiness.log",
    ),
    ctx,
  );
  let path = value_field(
    (
      "settings-service-ready-path",
      ctx.t("project_settings.services.editor.readiness.path"),
    ),
    (&sections.http_path, PATH_WIDTH),
    commit,
  )
  .placeholder(ctx.t("project_settings.services.editor.readiness.pathExample"));
  let pattern = value_field(
    (
      "settings-service-ready-pattern",
      ctx.t("project_settings.services.editor.readiness.pattern"),
    ),
    (&sections.log_pattern, PATTERN_WIDTH),
    commit,
  )
  .placeholder(ctx.t("project_settings.services.editor.readiness.patternExample"));
  let (path, pattern) = match (chosen, error.as_ref()) {
    (ReadyKind::Http, Some(_)) => (with_error(ctx, path, error.as_ref()), pattern),
    (ReadyKind::Log, Some(_)) => (path, with_error(ctx, pattern, error.as_ref())),
    _ => (path, pattern),
  };
  Column::new()
    .id("settings-service-ready-checks")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(port)
    .child(beside(http, path.render(ctx)))
    .child(beside(log, pattern.render(ctx)))
    .into()
}

/// A field `width` wide typing into `value`, saved as it is left.
fn value_field((id, name): (&str, Arc<str>), (value, width): (&Signal<String>, f32), commit: &Commit) -> FieldInput {
  input((id, name), value, commit).width(Dimension::Px(width))
}

/// A radio with its value's field beside it, 10 apart, the radio level with the field's first line (`TxJPu`).
pub(super) fn beside(radio: Element, value: Element) -> Element {
  Row::new()
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .height(Dimension::Px(ControlHeight::Md.value()))
        .justify(Justify::Center)
        .child(radio),
    )
    .child(value)
    .into()
}

/// *Timeout*: whole seconds, the unit after the number.
fn timeout_row(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.readiness.timeout");
  let error = view.problems().and_then(|problems| problems.timeout.as_ref());
  let control = input(("settings-service-timeout", label.clone()), &sections.timeout, commit)
    .width(Dimension::Px(TIMEOUT_WIDTH))
    .unit(ctx.t("project_settings.services.editor.readiness.seconds"));
  let control = with_error(ctx, control, error).render(ctx);
  field(
    "settings-service-timeout",
    label,
    ctx.t("project_settings.services.editor.readiness.timeoutHint"),
  )
  .render(ctx, control)
}
