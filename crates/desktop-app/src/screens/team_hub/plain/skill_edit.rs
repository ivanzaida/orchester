//! An admin's panel of a team skill, edited in place (`A28 · Agents · Skill · admin · editing` `qfrbn`): the head, the
//! chips with *Unsaved changes* while it differs from the version it is made on, *From the team* with its description
//! as a text area, its files and *Used by* (read-only: a profile gives it), *SKILL.md* as a text area, *On this
//! computer* (nothing) and the fixed Save/Cancel bar ([`super::super::edit`]). The fields are the form that makes one
//! ([`SkillFields`]); its reference files and where it is added stay as the version has them.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  node::Element,
};
use orchester_core::resource::Skill;
use orchester_workspace::agents::team_text::TeamSkillSheet;

use super::{Props, Sheet, from_team, on_this_computer, used_by_row};
use crate::{
  screens::{
    hub::forms::{Form, Ids, SkillFields},
    team_hub::{
      archive,
      edit::{Edit, View},
      panel, words,
    },
  },
  services::Services,
};

/// The ids of the edit's parts, which tests and automation address: `team-hub-from-team-description-input`,
/// `team-hub-skill-md-input`, …
const IDS: Ids = Ids {
  rows: "team-hub-from-team",
  sections: "team-hub",
};

#[derive(Clone, PartialEq)]
pub(super) struct EditorProps {
  pub props: Props,
  pub sheet: TeamSkillSheet,
}
impl DevtoolsInspectable for EditorProps {}

pub(super) struct SkillEditor {
  fields: SkillFields,
  edit: Edit<Skill>,
}

impl Component for SkillEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
    let fields = SkillFields::new(ctx);
    let load = {
      let fields = fields.clone();
      Arc::new(move |skill: &Skill| fields.load(skill))
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      sheet.edit.as_ref(),
      load,
    );
    Self { fields, edit }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
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
      sheet.edit.as_ref(),
      drawn.made.as_ref().map_or(true, |skill| *skill != base.body),
    );
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
    let head = panel::head_of(
      entry,
      (
        ctx.t("team_hub.kind.where.skill").to_string(),
        words::published_line(ctx, now, entry, sheet.published.as_ref()),
      ),
      archive::menu(ctx, &props.repo_root, entry, props.team.may_change),
    );
    let badges = parts
      .chip
      .into_iter()
      .chain([words::chip(ctx, &props.listing, entry)])
      .collect();
    let used_by = used_by_row(ctx, &sheet.used_by, Some(ctx.t("team_hub.edit.usedBySub")));
    let body = panel::body()
      .child(head)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .with_children(parts.notice)
      .child(from_team(ctx, drawn.rows.into_iter().chain([used_by]).collect()))
      .with_children(drawn.sections)
      .child(on_this_computer(ctx, &Sheet::Skill(sheet.clone())));
    panel::frame(entry, body, Some(parts.bar)).into()
  }
}
