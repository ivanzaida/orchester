//! The rows of what a profile runs with, the same in every form of it: the CLI, a local profile's connection (a select
//! of those of its CLI, each with the CLI's version its last check found, and *Add* beside it, which opens the
//! accounts; or, while this computer has none of its CLI, that it has none), the model and the effort.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{Runs, cli_value};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::{
    hub::profile_form::{Form, ProfileFields, Scope},
    team_hub::{
      profile::from_team::effort_words,
      rows::{self, Sub},
      words::cli_key,
    },
  },
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
  },
};

impl Runs {
  /// The rows: the CLI, a local profile's connection, the model and the effort.
  pub(in crate::screens::hub::profile_form) fn rows(
    &self,
    ctx: &mut Ctx,
    fields: &ProfileFields,
    form: &Form<'_>,
  ) -> Vec<Row> {
    let mut rows = vec![self.cli_row(ctx, fields, form)];
    if form.scope == Scope::Local {
      rows.push(self.connection_row(ctx, fields, form));
    }
    rows.push(self.model_row(ctx, fields, form));
    rows.push(self.effort_row(ctx, fields, form));
    rows
  }

  fn cli_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let options = self
      .clis
      .iter()
      .map(|cli| SelectOption::new(cli_value(*cli), ctx.t(cli_key(*cli)).to_string()))
      .collect();
    // A team's CLI is a kind, never a connection, which its line says; a local profile names its connection apart.
    let quiet = (form.scope == Scope::Team).then(|| ctx.t("team_hub.profile.agentCliSub"));
    let missing = form.tried && self.cli.is_none();
    let spec = Spec {
      id: "cli",
      label: "team_hub.profile.agentCli",
      placeholder: ctx.t("team_new.profile.cliPlaceholder"),
    };
    let select = select(ctx, form, (&spec, &fields.cli, options), missing);
    row(ctx, form, &spec, said(ctx, missing, quiet), select.render(ctx))
  }

  /// *Connection* of a local profile: before its CLI is chosen, that it is chosen with it; while this computer has no
  /// connection of its CLI, that it has none; else a select of those it has, with *Add* for another.
  fn connection_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let Some(cli) = self.cli else {
      let spec = Spec {
        id: "connection",
        label: "local_new.profile.connection",
        placeholder: ctx.t("local_new.profile.connectionAuto"),
      };
      let select = select(ctx, form, (&spec, &fields.connection, Vec::new()), false).locked();
      let quiet = Some(ctx.t("local_new.profile.connectionSub"));
      return row(ctx, form, &spec, quiet.map(Sub::quiet), select.render(ctx));
    };
    let cli_words = ctx.t(cli_key(cli));
    let spec = Spec {
      id: "connection",
      label: "local_new.profile.connection",
      placeholder: ctx.t_args("team_hub.profile.choose", [("cli", &*cli_words)]),
    };
    if self.of_kind.is_empty() {
      return row(ctx, form, &spec, None, none_here(ctx, form, &cli_words));
    }
    let missing = form.tried && self.connection.is_none();
    let options = self.connection_options(ctx, cli, &cli_words);
    let select = select(ctx, form, (&spec, &fields.connection, options), missing);
    let add = ActionButton::new(
      format!("{}-add-connection", form.ids.rows),
      ButtonKind::Ghost,
      ctx.t("local_hub.profile.addConnection"),
    )
    .icon(Icon::Plus)
    .small()
    .opens(Navigation::of(ctx).to(Route::Accounts(AccountsView::default())));
    let value = Row::new()
      .id(format!("{}-connection-value", form.ids.rows))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(select.render(ctx)),
      )
      .child(add);
    let sub = if missing { required(ctx) } else { self.of_its_kind(ctx) };
    row(ctx, form, &spec, sub, value)
  }

  /// Each connection of the CLI as *Claude Code · Home*, with the CLI's version its last check found.
  fn connection_options(
    &self,
    ctx: &Ctx,
    cli: orchester_core::connection::AgentProvider,
    cli_words: &str,
  ) -> Vec<SelectOption<String>> {
    let program = cli.program_name();
    self
      .of_kind
      .iter()
      .map(|choice| {
        let option = SelectOption::new(
          choice.id.to_string(),
          ctx.t_args(
            "team_hub.profile.connectionName",
            [("cli", cli_words), ("name", choice.name.as_str())],
          ),
        );
        match (&choice.version, program) {
          (Some(version), Some(program)) => option.detail(ctx.t_args(
            "team_hub.profile.cliVersion",
            [("program", program), ("version", version)],
          )),
          _ => option,
        }
      })
      .collect()
  }

  /// *2 of its kind*, while it has several to choose from or none chosen; in `warning` while none is.
  fn of_its_kind(&self, ctx: &Ctx) -> Option<Sub> {
    let many = self.of_kind.len();
    (many > 1 || self.connection.is_none()).then(|| Sub {
      words: ctx.t_args("local_hub.profile.ofItsKind", [("count", many.to_string())]),
      color: if self.connection.is_none() {
        PaletteColor::Warning
      } else {
        PaletteColor::TextMuted
      },
    })
  }

  /// *Model*: the models of the connection, an identifier each. A local profile's waits for its connection.
  fn model_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let options = self
      .models
      .iter()
      .map(|model| SelectOption::new(model.clone(), model.clone()))
      .collect();
    let spec = Spec {
      id: "model",
      label: "team_hub.profile.model",
      placeholder: ctx.t("team_new.profile.modelPlaceholder"),
    };
    let missing = form.tried && self.model.is_none();
    let waits = self.models.is_empty() || (form.scope == Scope::Local && self.connection.is_none());
    let mut select = select(ctx, form, (&spec, &fields.model, options), missing).mono();
    if waits {
      select = select.locked();
    }
    row(ctx, form, &spec, said(ctx, missing, None), select.render(ctx))
  }

  /// *Reasoning effort*: the CLI's default, or one its model lists.
  fn effort_row(&self, ctx: &mut Ctx, fields: &ProfileFields, form: &Form<'_>) -> Row {
    let default = ctx.t("team_new.profile.effortDefault");
    let mut options = vec![SelectOption::new(String::new(), default.to_string())];
    options.extend(
      self
        .efforts
        .iter()
        .map(|effort| SelectOption::new(effort.clone(), effort_words(ctx, effort))),
    );
    let spec = Spec {
      id: "effort",
      label: "team_hub.profile.effort",
      placeholder: default,
    };
    let mut select = select(ctx, form, (&spec, &fields.effort, options), false);
    if self.efforts.is_empty() {
      select = select.locked();
    }
    row(ctx, form, &spec, None, select.render(ctx))
  }
}

/// What the connection's row says while this computer has no connection of the CLI: *No Claude Code connection on
/// this computer*, in `danger`.
fn none_here(ctx: &Ctx, form: &Form<'_>, cli: &str) -> Element {
  Row::new()
    .id(format!("{}-connection-value", form.ids.rows))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleX, IconSize::Base, PaletteColor::Danger))
    .child(
      Text::new(&ctx.t_args("team_hub.profile.noConnection", [("cli", cli)]))
        .variant(HubText::Value.role())
        .color(PaletteColor::Danger),
    )
    .into()
}

/// A select's id after the form's, and the key of its label and what it says while nothing is chosen.
struct Spec {
  id: &'static str,
  label: &'static str,
  placeholder: Arc<str>,
}

/// The select `<rows>-<id>` of `options` holding `value`; `missing` outlines it in `danger`. Locked while the form
/// is.
fn select(
  ctx: &Ctx,
  form: &Form<'_>,
  (spec, value, options): (&Spec, &Signal<String>, Vec<SelectOption<String>>),
  missing: bool,
) -> SelectField<String> {
  let mut select = SelectField::new(
    format!("{}-{}", form.ids.rows, spec.id),
    ctx.t(spec.label),
    value.clone(),
    options,
  )
  .placeholder(spec.placeholder.clone());
  if missing {
    select = select.invalid();
  }
  if form.locked {
    select = select.locked();
  }
  select
}

/// The row `<rows>-<id>-row` of `value`, with `sub` under its label.
fn row(ctx: &Ctx, form: &Form<'_>, spec: &Spec, sub: Option<Sub>, value: impl Into<Element>) -> Row {
  rows::row(
    &format!("{}-{}-row", form.ids.rows, spec.id),
    &ctx.t(spec.label),
    sub,
    value,
  )
}

/// What a row says under its label: that it is required in `danger` while `missing`, else `quiet`.
fn said(ctx: &Ctx, missing: bool, quiet: Option<Arc<str>>) -> Option<Sub> {
  if missing { required(ctx) } else { quiet.map(Sub::quiet) }
}

fn required(ctx: &Ctx) -> Option<Sub> {
  Some(Sub {
    words: ctx.t("team_new.errors.required"),
    color: PaletteColor::Danger,
  })
}
