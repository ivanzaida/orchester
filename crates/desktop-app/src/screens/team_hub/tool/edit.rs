//! An admin's panel of a team tool, edited in place (`A29 · Agents · Tool · admin · editing` `FrraS` for a command,
//! A33 `t2RMXK` for an HTTP request, A35 `YBG4T` for a database query): the head, the chips with *Unsaved changes*
//! while it differs from the version it is made on, *From the team* with the form that makes one ([`ToolFields`]: its
//! kind, description, and what it runs or calls), an HTTP or database tool's *Access*, *On this computer* as the plain
//! panel has it, and the fixed Save/Cancel bar ([`super::super::edit`]). A command's exit codes, output limit and
//! environment stay as the version has them. The value of the secret a tool names is given here by pressing its box,
//! since the bar is Save's: it is the person's, as the plain panel's *Add value* is (A6 `u9Hcc`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  node::Element,
};
use orchester_core::{resource::TeamTool, secret::SecretName};
use orchester_workspace::agents::team_tool::{ToolSecret, calling_pipelines};

use super::{Props, chip, computer, from_team};
use crate::{
  screens::{
    hub::forms::{Form, Ids, ToolFields},
    settings::orchester::PersonOnly,
    team_hub::{
      actions::Actions,
      archive,
      edit::{Edit, View},
      panel, rows, secret, words,
    },
  },
  services::Services,
  ui::pressable::Press,
};

/// The ids of the edit's parts, which tests and automation address: `team-hub-from-team-program-input`,
/// `team-hub-from-team-url-input`, `team-hub-from-team-environment-value-production`, …
const IDS: Ids = Ids {
  rows: "team-hub-from-team",
  sections: "team-hub",
};

#[derive(Clone, PartialEq)]
pub(super) struct EditorProps {
  pub props: Props,
}
impl DevtoolsInspectable for EditorProps {}

pub(super) struct ToolEditor {
  fields: ToolFields,
  edit: Edit<TeamTool>,
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
}

impl Component for ToolEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let fields = ToolFields::new(ctx);
    let load = {
      let fields = fields.clone();
      Arc::new(move |tool: &TeamTool| fields.load_team(tool))
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      props.sheet.latest.as_ref(),
      load,
    );
    let editor = Self {
      fields,
      edit,
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
    };
    editor.secret.follow_save(ctx, &editor.actions.add_value);
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let Some(base) = self.edit.base() else {
      return Element::from(Row::new());
    };
    let pipelines = Services::of(ctx)
      .workspace
      .team_read(&props.repo_root)
      .config()
      .map_or_else(Vec::new, |config| calling_pipelines(config));
    let drawn = self.fields.draw_team(
      ctx,
      &Form {
        ids: IDS,
        base: Some(&base.body),
        name: None,
        tried: true,
        locked: false,
      },
      &pipelines,
    );
    self.edit.keep_up(
      props.sheet.latest.as_ref(),
      drawn.made.as_ref().map_or(true, |tool| *tool != base.body),
    );
    let sheet = &props.sheet;
    let view = View {
      made: drawn.made,
      unreached: props.unreached.as_ref(),
      published_by: sheet.published.as_ref().map(|published| published.by.as_str()),
    };
    let Some(parts) = self.edit.parts(ctx, view) else {
      return Row::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let entry = &props.entry;
    let kind = ctx.t_args(
      "team_hub.tool.where",
      [("kind", from_team::kind(ctx, sheet).to_string())],
    );
    let head = panel::head_of(
      entry,
      (
        kind.to_string(),
        words::published_line(ctx, now, entry, sheet.published.as_ref()),
      ),
      archive::menu(ctx, &props.repo_root, entry, props.team.may_change),
    );
    let badges = parts
      .chip
      .into_iter()
      .chain([chip(ctx, &props.listing, entry)])
      .collect();
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let ask = match &sheet.secret {
      ToolSecret::Named(name) => Some(self.ask(name)),
      ToolSecret::ById | ToolSecret::None => None,
    };
    let body = panel::body()
      .child(head)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .with_children(parts.notice)
      .child(rows::from_team(ctx, rows::FromTeam::Shared, drawn.rows))
      .with_children(drawn.sections)
      .child(computer::section(ctx, (entry, sheet), missing, ask));
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
    );
    panel::frame(entry, body, Some(parts.bar)).with_children(dialog).into()
  }
}

impl Drop for ToolEditor {
  fn drop(&mut self) {
    self.secret.wipe();
  }
}

impl ToolEditor {
  /// A press on the box of the value of `name`: the dialog that takes it opens, on a person's press alone.
  fn ask(&self, name: &SecretName) -> Press {
    let (entry, name, person) = (self.secret.clone(), name.clone(), self.person.clone());
    Press::run(move || {
      if person.allows("fix what a team tool needs on this computer") {
        entry.ask(&name);
      }
    })
  }
}
