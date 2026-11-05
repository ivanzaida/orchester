//! Artifacts (`Artifacts` `i8I1Kr`): *Keep*, *Add a path pattern* with *Add* beside it (`Entry row` `N0I8qE`), the
//! patterns as pills with their × (`Pills` `xjblv`), and that the logs of the services it needed are always kept.
//! Limits (`Limits` `VdriF`): the timeout in a 120 field with *minutes* after the number (`Urydj`), the filter (the one
//! argument `run_tests` fills with what it narrows the suite to, `kkuLY`), and whether every Test step runs it, as a
//! switch (`IZLFy`). Adding or removing a pattern and flipping the switch save at once, a field as it is left.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{
  super::{
    super::kit::{Group, SettingRow, form_note, on_enter},
    editor::fields::{field, input, with_error},
  },
  Commit, View,
  fields::SuiteFields,
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::FieldInput,
    icon::Icon,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The timeout's field (`Input` `Urydj`).
const TIMEOUT_WIDTH: f32 = 120.0;
/// What stands in the filter's argument: an identifier, not words.
const FILTER: &str = "{{filter}}";

/// The Artifacts group.
pub(super) fn artifacts_group(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let artifacts = fields.choices.get().artifacts;
  let add = adder(fields, commit);
  let entry = FieldInput::new(
    "settings-suite-artifact-add".to_owned(),
    ctx.t("project_settings.services.suite.artifactAdd"),
    fields.artifact.clone(),
  )
  .mono()
  .placeholder(ctx.t("project_settings.services.suite.artifactAdd"));
  let entry = match view.problems().and_then(|problems| problems.artifacts.as_ref()) {
    Some(error) => entry.error(message_text(ctx, error)),
    None => entry,
  };
  let line = Row::new()
    .id("settings-suite-artifact-entry")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .on_key_down(on_enter({
      let add = add.clone();
      move || add()
    }))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(entry.render(ctx)),
    )
    .child(Element::from(
      ActionButton::new(
        "settings-suite-artifact-add-button",
        ButtonKind::Secondary,
        ctx.t("project_settings.services.editor.owned.addButton"),
      )
      .icon(Icon::Plus)
      .on_click(move || add()),
    ));
  let pills = (!artifacts.is_empty()).then(|| {
    Row::new()
      .id("settings-suite-artifacts")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .with_children(
        artifacts
          .iter()
          .enumerate()
          .map(|(at, glob)| pill(ctx, fields, (at, glob), commit)),
      )
  });
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(line)
    .with_children(pills)
    .child(form_note(
      "settings-suite-artifacts-note",
      &ctx.t("project_settings.services.suite.logsKept"),
    ));
  let row = field(
    "settings-suite-artifacts",
    ctx.t("project_settings.services.suite.keep"),
    ctx.t("project_settings.services.suite.keepHint"),
  )
  .render(ctx, control);
  Group::new(
    "settings-suite-artifacts-group",
    ctx.t("project_settings.services.suite.artifacts"),
  )
  .row(row)
  .into()
}

/// Adds the pattern typed after the others and saves; a blank one, or one kept already, adds nothing.
fn adder(fields: &SuiteFields, commit: &Commit) -> Arc<dyn Fn() + Send + Sync> {
  let (fields, commit) = (fields.clone(), commit.clone());
  Arc::new(move || {
    let typed = fields.artifact.get_untracked().trim().to_owned();
    if typed.is_empty() {
      return;
    }
    fields.artifact.set(String::new());
    fields.choose(|choices| {
      if !choices.artifacts.contains(&typed) {
        choices.artifacts.push(typed);
      }
    });
    commit();
  })
}

/// A kept pattern's pill; its × removes it and saves.
fn pill(ctx: &Ctx, fields: &SuiteFields, (at, glob): (usize, &str), commit: &Commit) -> Element {
  let (fields, commit) = (fields.clone(), commit.clone());
  TagChip::new(format!("settings-suite-artifact-{at}"), glob.to_owned())
    .mono()
    .pill()
    .removable(
      ctx.t_args("project_settings.services.editor.owned.remove", [("path", glob)]),
      move || {
        fields.choose(|choices| {
          if at < choices.artifacts.len() {
            choices.artifacts.remove(at);
          }
        });
        commit();
      },
    )
    .into()
}

/// The Limits group.
pub(super) fn limits_group(ctx: &mut Ctx, fields: &SuiteFields, view: &View, commit: &Commit) -> Element {
  let problems = view.problems();
  let label = ctx.t("project_settings.services.suite.timeout");
  let control = input(("settings-suite-timeout", label.clone()), &fields.timeout, commit)
    .width(Dimension::Px(TIMEOUT_WIDTH))
    .unit(ctx.t("project_settings.services.suite.minutes"));
  let control = with_error(ctx, control, problems.and_then(|p| p.timeout.as_ref())).render(ctx);
  let timeout = field(
    "settings-suite-timeout",
    label,
    ctx.t("project_settings.services.suite.timeoutHint"),
  )
  .render(ctx, control);
  let label = ctx.t("project_settings.services.suite.filter");
  // The example names the placeholder as its value, so the locale does not read it as one of its own.
  let example = ctx.t_args("project_settings.services.suite.filterExample", [("filter", FILTER)]);
  let control = input(("settings-suite-filter", label.clone()), &fields.filter, commit).placeholder(example);
  let control = with_error(ctx, control, problems.and_then(|p| p.filter.as_ref())).render(ctx);
  let filter = field(
    "settings-suite-filter",
    label,
    ctx.t("project_settings.services.suite.filterHint"),
  )
  .render(ctx, control);
  let always = fields.choices.get().always;
  let (choose, commit) = (fields.clone(), commit.clone());
  let switch = Toggle::new(
    "settings-suite-always-switch",
    ToggleKind::Switch,
    always,
    ctx.t("project_settings.services.suite.always"),
  )
  .hide_label()
  .on_change(move |on| {
    choose.choose(|choices| choices.always = on);
    commit();
  });
  let always = SettingRow::new("settings-suite-always", ctx.t("project_settings.services.suite.always"))
    .hint(ctx.t("project_settings.services.suite.alwaysHint"))
    .control(switch)
    .render(ctx);
  Group::new(
    "settings-suite-limits-group",
    ctx.t("project_settings.services.suite.limits"),
  )
  .row(timeout)
  .row(filter)
  .row(always)
  .into()
}
