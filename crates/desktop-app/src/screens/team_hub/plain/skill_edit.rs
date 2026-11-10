//! An admin's panel of a team skill, edited in place (`A28 · Agents · Skill · admin · editing` `qfrbn`): the head, the
//! chips with *Unsaved changes* while it differs from the version it is made on, *From the team* with its description
//! as a text area, its files and *Used by* (read-only: a profile gives it), *SKILL.md* as a text area, *On this
//! computer* (nothing) and the fixed Save/Cancel bar ([`super::super::edit`]). Its reference files and where it is added
//! stay as the version has them: the files are not changed here, as in a local skill's panel.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_core::resource::Skill;
use orchester_workspace::agents::team_text::TeamSkillSheet;

use super::{Props, Sheet, from_team, on_this_computer, used_by_row};
use crate::{
  screens::{
    hub::area::Area,
    team_hub::{
      archive,
      edit::{Edit, TextSection, View},
      panel, rows, words,
    },
  },
  services::Services,
};

/// The description is at least 64 high, as the form that makes one draws it (`Input` in A11 `p4aSf`).
const DESCRIPTION_AREA: f32 = 64.0;
/// SKILL.md is at least 180 high, as an instruction's text.
const BODY_AREA: f32 = 180.0;

#[derive(Clone, PartialEq)]
pub(super) struct EditorProps {
  pub props: Props,
  pub sheet: TeamSkillSheet,
}
impl DevtoolsInspectable for EditorProps {}

pub(super) struct SkillEditor {
  description: Signal<String>,
  body: Signal<String>,
  edit: Edit<Skill>,
}

impl Component for SkillEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
    let (description, body) = (ctx.signal(String::new()), ctx.signal(String::new()));
    let load = {
      let (description, body) = (description.clone(), body.clone());
      Arc::new(move |skill: &Skill| {
        description.set(skill.description().to_owned());
        body.set(skill.body().to_owned());
      })
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      sheet.edit.as_ref(),
      load,
    );
    Self {
      description,
      body,
      edit,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
    let Some(base) = self.edit.base() else {
      return Element::from(Row::new());
    };
    let (description, text) = (self.description.get(), self.body.get());
    let made = made(ctx, (&description, &text), &base.body);
    self.edit.keep_up(
      sheet.edit.as_ref(),
      made.as_ref().map_or(true, |skill| *skill != base.body),
    );
    let description_error = description
      .trim()
      .is_empty()
      .then(|| ctx.t("team_hub.edit.descriptionRequired"));
    let body_error = text.trim().is_empty().then(|| ctx.t("team_hub.edit.textRequired"));
    let view = View {
      made,
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
    let description_row = Area {
      id: "team-hub-from-team-description".to_owned(),
      label: ctx.t("team_hub.plain.description"),
      sub: ctx.t("team_hub.edit.descriptionSub"),
      placeholder: ctx.t("team_hub.edit.descriptionPlaceholder"),
      height: DESCRIPTION_AREA,
    }
    .row(ctx, &self.description, description_error);
    let files = rows::row(
      "team-hub-from-team-files",
      &ctx.t("team_hub.plain.files"),
      Some(rows::Sub::quiet(ctx.t("team_hub.edit.filesSub"))),
      rows::tags("team-hub-from-team-files-value", sheet.files.iter().cloned()),
    );
    let used_by = used_by_row(ctx, &sheet.used_by, Some(ctx.t("team_hub.edit.usedBySub")));
    let document = TextSection {
      id: "team-hub-skill-md",
      label: ctx.t("team_hub.edit.skillDocument"),
      placeholder: ctx.t("team_hub.edit.skillDocumentPlaceholder"),
      height: BODY_AREA,
    }
    .render(ctx, &self.body, (text != base.body.body(), body_error));
    let body = panel::body()
      .child(head)
      .with_children(parts.notice)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .child(from_team(ctx, vec![description_row, files, used_by]))
      .child(document)
      .child(on_this_computer(ctx, &Sheet::Skill(sheet.clone())));
    panel::frame(entry, body, Some(parts.bar)).into()
  }
}

/// The skill the fields make: `description` and SKILL.md's `text` in the place of `base`'s, which keeps its files and
/// where it is added. A blank description or text makes none, and says why.
fn made(ctx: &Ctx, (description, text): (&str, &str), base: &Skill) -> Result<Skill, Arc<str>> {
  if description.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.descriptionRequired"));
  }
  if text.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.textRequired"));
  }
  Ok(
    Skill::new(
      description.to_owned(),
      text.to_owned(),
      base.files().to_vec(),
      base.scope().clone(),
    )
    .expect("its files are the published version's own, each with its own path"),
  )
}
