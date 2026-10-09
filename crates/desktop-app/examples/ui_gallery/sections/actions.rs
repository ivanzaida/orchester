//! Buttons (`Button/*` in `ASNYj`), their loading, disabled and unavailable states, icon buttons and
//! the unavailable reason line.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Row,
  node::Element,
};
use orchester_app::ui::{
  button::{ActionButton, ButtonKind},
  icon::Icon,
  icon_button::IconButton,
  unavailable::{reason_line, with_reason},
};

use super::{cell, line, stack};
use crate::gallery::State;

const KINDS: [(&str, ButtonKind, &str, &str, Icon); 4] = [
  (
    "primary",
    ButtonKind::Primary,
    "Button/primary · yJ2jo",
    "New task",
    Icon::Plus,
  ),
  (
    "secondary",
    ButtonKind::Secondary,
    "Button/secondary · Dswcx",
    "Review runs",
    Icon::History,
  ),
  (
    "ghost",
    ButtonKind::Ghost,
    "Button/ghost · cnlv1",
    "Manage tags",
    Icon::Tags,
  ),
  (
    "danger",
    ButtonKind::Danger,
    "Button/danger · O3M8iH",
    "Delete",
    Icon::Trash2,
  ),
];

pub fn section(ctx: &mut Ctx, state: &State) -> Element {
  let press = |what: &'static str| {
    let pressed = state.pressed.clone();
    move || pressed.set(what.to_owned())
  };
  let with_icon = KINDS.map(|(id, kind, caption, label, glyph)| {
    cell(
      caption,
      ActionButton::new(format!("g-{id}"), kind, label)
        .icon(glyph)
        .on_click(press(label)),
    )
  });
  let plain = KINDS.map(|(id, kind, _, label, _)| {
    cell(
      "without icon",
      ActionButton::new(format!("g-{id}-plain"), kind, label).on_click(press(label)),
    )
  });
  let states = [
    cell(
      "loading · the request is under way",
      ActionButton::new("g-loading", ButtonKind::Primary, "Create & run")
        .icon(Icon::Play)
        .loading(),
    ),
    cell(
      "disabled",
      ActionButton::new("g-disabled", ButtonKind::Secondary, "Save changes").disabled(),
    ),
    cell(
      "unavailable with its reason · Q9aeZ, Nt3Qi",
      unavailable_actions(press("New task")),
    ),
  ];
  let form = Row::new()
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new("g-form-save", ButtonKind::Primary, "Create")
        .in_form()
        .on_click(press("Create")),
    )
    .child(
      IconButton::new("g-form-remove", Icon::X, "Remove criterion")
        .small()
        .in_form()
        .on_click(press("Remove criterion"))
        .render(ctx),
    );
  let icons = [
    cell(
      "Button/icon · JJEzp",
      IconButton::new("g-icon", Icon::Settings, "Settings")
        .on_click(press("Settings"))
        .render(ctx),
    ),
    cell(
      "small · Remove in o6h0L5",
      IconButton::new("g-icon-small", Icon::X, "Remove entry")
        .small()
        .on_click(press("Remove entry"))
        .render(ctx),
    ),
    cell("in a form · Tab reaches them, the focus ring shows", form),
    cell(
      "Unavailable reason · QR1ER",
      reason_line(
        "g-reason",
        "Review PR needs a pull request, which no pipeline step opens yet.",
      ),
    ),
  ];
  stack([
    line(with_icon).into(),
    line(plain).into(),
    line(states).into(),
    line(icons).into(),
  ])
}

/// The Tasks header's actions (`Actions` `mQeh3`): Create multiple unavailable, New task, and why.
fn unavailable_actions(new_task: impl Fn() + Send + Sync + 'static) -> Element {
  let buttons = Row::new()
    .spacing(SpacingSize::Sm)
    .child(
      ActionButton::new("g-multiple", ButtonKind::Secondary, "Create multiple")
        .icon(Icon::Plus)
        .unavailable(),
    )
    .child(
      ActionButton::new("g-new-task", ButtonKind::Primary, "New task")
        .icon(Icon::Plus)
        .on_click(new_task),
    );
  with_reason(
    buttons,
    "g-multiple-reason",
    "Creating several tasks at once is planned.",
  )
  .into()
}
