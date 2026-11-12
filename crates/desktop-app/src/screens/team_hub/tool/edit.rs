//! An admin's panel of a team command tool, edited in place (`A29 · Agents · Tool · admin · editing` `FrraS`): the
//! head, the chips with *Unsaved changes* while it differs from the version it is made on, *From the team* with the
//! form that makes one ([`ToolFields`]: its kind, description, program, arguments one per line and how long it may
//! run), *On this computer* as the plain panel has it, and the fixed Save/Cancel bar ([`super::super::edit`]). Its exit
//! codes, output limit and environment stay as the version has them. An HTTP request or a database query is read
//! here as values and not edited (A6 `u9Hcc`): its panel stays the plain one.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  node::Element,
};
use orchester_core::resource::Tool;

use super::{Props, chip, computer, from_team};
use crate::{
  screens::{
    hub::forms::{Form, Ids, ToolFields},
    team_hub::{
      archive,
      edit::{Edit, View},
      panel, words,
    },
  },
  services::Services,
};

/// The ids of the edit's parts, which tests and automation address: `team-hub-from-team-program-input`,
/// `team-hub-from-team-arguments-input`, …
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
  edit: Edit<Tool>,
}

impl Component for ToolEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let fields = ToolFields::new(ctx);
    let load = {
      let fields = fields.clone();
      Arc::new(move |tool: &Tool| fields.load(tool))
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      props.sheet.edit.as_ref(),
      load,
    );
    Self { fields, edit }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props } = ctx.props::<EditorProps>().clone();
    let Some(base) = self.edit.base() else {
      return Element::from(Row::new());
    };
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: Some(&base.body),
        name: None,
        tried: true,
        locked: false,
      },
    );
    self.edit.keep_up(
      props.sheet.edit.as_ref(),
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
    let body = panel::body()
      .child(head)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .with_children(parts.notice)
      .child(from_team::section_of(ctx, drawn.rows))
      .child(computer::section(ctx, entry, sheet, &[]));
    panel::frame(entry, body, Some(parts.bar)).into()
  }
}
