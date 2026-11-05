//! The variables of a service's Program group (`Variables` `cVZui`) and of a test suite's Run group (`Variables`
//! `HGnzG`): each a row of its name in a 190 field, `=` and its value, or the name of a project secret after a key
//! (`Var SESSION_SECRET` `sTrHF`), then *Add variable* and *Add secret* (`Add variable` `E9kFqw`). A row leaves the
//! variables when its name and value are both cleared. A secret's value is never read here: its row holds the secret's
//! name only.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{agent_testing::editor::form::VariableLine, messages::Message};

use super::{Commit, fields::Lines};
use crate::{
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
  },
};

/// A variable's name field (`Input` `KEqts`).
const NAME_WIDTH: f32 = 190.0;
/// What stands between a variable's name and its value (`Text` `KnQ0G`): a sign, not words.
const EQUALS: &str = "=";

/// The variables' rows, the adders and `error` under them; their ids start with `prefix` (`settings-service`).
pub(in crate::screens::project_settings::services) fn variables(
  ctx: &mut Ctx,
  (prefix, variables): (&str, &Signal<Lines>),
  error: Option<&Message>,
  commit: &Commit,
) -> Element {
  let lines = variables.get().0;
  let rows: Vec<Element> = lines
    .into_iter()
    .enumerate()
    .map(|(at, line)| {
      ctx.mount_keyed::<VariableRow>(
        &format!("{prefix}-variable-{at}"),
        RowProps {
          id: format!("{prefix}-variable-{at}").into(),
          at,
          line,
          variables: variables.clone(),
          commit: commit.clone(),
        },
      )
    })
    .collect();
  let adders = Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(adder(ctx, (prefix, variables), false))
    .child(adder(ctx, (prefix, variables), true));
  Column::new()
    .id(format!("{prefix}-variables"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .with_children(rows)
    .child(adders)
    .with_children(error.map(|error| {
      Element::from(field_error(
        &format!("{prefix}-variables-error"),
        &message_text(ctx, error),
      ))
    }))
    .into()
}

/// *Add variable*, or *Add secret* when `secret`: a new empty row of that kind.
fn adder(ctx: &Ctx, (prefix, variables): (&str, &Signal<Lines>), secret: bool) -> Element {
  let (id, key, glyph) = if secret {
    (
      format!("{prefix}-add-secret"),
      "project_settings.services.editor.addSecret",
      Icon::KeyRound,
    )
  } else {
    (
      format!("{prefix}-add-variable"),
      "project_settings.services.editor.addVariable",
      Icon::Plus,
    )
  };
  let variables = variables.clone();
  ActionButton::new(id, ButtonKind::Ghost, ctx.t(key))
    .icon(glyph)
    .small()
    .on_click(move || {
      variables.update(|lines| {
        lines.0.push(VariableLine {
          secret,
          ..VariableLine::default()
        });
      });
    })
    .into()
}

#[derive(Clone)]
struct RowProps {
  /// The row's element id.
  id: Arc<str>,
  at: usize,
  line: VariableLine,
  variables: Signal<Lines>,
  commit: Commit,
}

/// Compared by what the row shows; the signal and the save are the page's.
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.at == other.at && self.line == other.line
  }
}

impl DevtoolsInspectable for RowProps {}

/// A variable's row: its two fields type into the page's variables at its place.
struct VariableRow {
  name: Signal<String>,
  value: Signal<String>,
}

impl Component for VariableRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<RowProps>().clone();
    let row = Self {
      name: ctx.signal(props.line.name.clone()),
      value: ctx.signal(props.line.value.clone()),
    };
    for (part, value) in [(&row.name, false), (&row.value, true)] {
      let (variables, at) = (props.variables.clone(), props.at);
      ctx.watch(part, move |typed: &String| {
        variables.update(|lines| {
          if let Some(line) = lines.0.get_mut(at) {
            let slot = if value { &mut line.value } else { &mut line.name };
            slot.clone_from(typed);
          }
        });
      });
    }
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RowProps {
      id, at, line, commit, ..
    } = ctx.props::<RowProps>().clone();
    // The page's variables move only through this row's fields once it shows, but a fill (the page read again) puts
    // what is stored in them.
    if self.name.get_untracked() != line.name {
      self.name.set(line.name.clone());
    }
    if self.value.get_untracked() != line.value {
      self.value.set(line.value.clone());
    }
    let leave = |commit: &Commit| {
      let commit = commit.clone();
      move || commit()
    };
    let name = FieldInput::new(
      format!("{id}-name"),
      ctx.t_args(
        "project_settings.services.editor.variableName",
        [("number", (at + 1).to_string())],
      ),
      self.name.clone(),
    )
    .mono()
    .width(Dimension::Px(NAME_WIDTH))
    .on_leave(leave(&commit));
    let value = FieldInput::new(format!("{id}-value"), value_name(ctx, &line), self.value.clone())
      .mono()
      .on_leave(leave(&commit));
    let value = if line.secret {
      value
        .leading(Icon::KeyRound)
        .placeholder(ctx.t("project_settings.services.editor.secret"))
    } else {
      value
    };
    Row::new()
      .id(id.to_string())
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(name.render(ctx))
      .child(Text::new(EQUALS).variant(ProjectSettingsText::Hint.role()))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(value.render(ctx)),
      )
  }
}

/// What tools call a row's value field: the value of its variable, or the secret it takes.
fn value_name(ctx: &Ctx, line: &VariableLine) -> Arc<str> {
  let key = if line.secret {
    "project_settings.services.editor.secretOf"
  } else {
    "project_settings.services.editor.variableValue"
  };
  ctx.t_args(key, [("name", line.name.clone())])
}
