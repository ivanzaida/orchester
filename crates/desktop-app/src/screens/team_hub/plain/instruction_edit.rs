//! An admin's panel of a team instruction, edited in place (`A27 · Agents · Instruction · admin · editing` `uVUZ4`):
//! the head, the chips with *Unsaved changes* while the text differs from the version it is made on, *From the team*
//! with *Used by* (read-only: a profile gives it, and is changed there) and its format, *Text* as a text area, *On this
//! computer* (nothing) and the fixed Save/Cancel bar ([`super::super::edit`]). Who gets it, when and where it is added
//! stay as the version has them.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::resource::{Instruction, PromptText};
use orchester_workspace::agents::{form::prompt_error, team_text::TeamInstructionSheet};

use super::{Props, Sheet, from_team, on_this_computer, used_by_row};
use crate::{
  screens::team_hub::{
    archive,
    edit::{Edit, TextSection, View},
    panel, rows, words,
  },
  services::Services,
  ui::message::message_text,
};

/// The text area is at least 180 high, as the form that makes one draws it (`Input` `sTxOp` in A10 `hGdGu`).
const TEXT_AREA: f32 = 180.0;

#[derive(Clone, PartialEq)]
pub(super) struct EditorProps {
  pub props: Props,
  pub sheet: TeamInstructionSheet,
}
impl DevtoolsInspectable for EditorProps {}

pub(super) struct InstructionEditor {
  text: Signal<String>,
  edit: Edit<Instruction>,
}

impl Component for InstructionEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
    let text = ctx.signal(String::new());
    let load = {
      let text = text.clone();
      Arc::new(move |body: &Instruction| text.set(body.text().as_str().to_owned()))
    };
    let edit = Edit::new(
      ctx,
      (props.repo_root.clone(), props.project, props.entry.id),
      sheet.edit.as_ref(),
      load,
    );
    Self { text, edit }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, sheet } = ctx.props::<EditorProps>().clone();
    let Some(base) = self.edit.base() else {
      return Element::from(lurq::components::Row::new());
    };
    let text = self.text.get();
    let made = made(ctx, &text, &base.body);
    self.edit.keep_up(
      sheet.edit.as_ref(),
      made.as_ref().map_or(true, |body| *body != base.body),
    );
    let error = made.as_ref().err().cloned();
    let view = View {
      made,
      unreached: props.unreached.as_ref(),
      published_by: sheet.published.as_ref().map(|published| published.by.as_str()),
    };
    let Some(parts) = self.edit.parts(ctx, view) else {
      return lurq::components::Row::new().into();
    };
    let now = Services::of(ctx).clock.now();
    let entry = &props.entry;
    let head = panel::head_of(
      entry,
      (
        ctx.t("team_hub.kind.where.instruction").to_string(),
        words::published_line(ctx, now, entry, sheet.published.as_ref()),
      ),
      archive::menu(ctx, &props.repo_root, entry, props.team.may_change),
    );
    let badges = parts
      .chip
      .into_iter()
      .chain([words::chip(ctx, &props.listing, entry)])
      .collect();
    let format = rows::row(
      "team-hub-from-team-format",
      &ctx.t("team_hub.plain.format"),
      None,
      rows::words("team-hub-from-team-format-value", &ctx.t("team_hub.plain.markdown")),
    );
    let section = TextSection {
      id: "team-hub-text",
      label: ctx.t("team_hub.plain.text"),
      placeholder: ctx.t("team_hub.edit.textPlaceholder"),
      height: TEXT_AREA,
    }
    .render(ctx, &self.text, (text != base.body.text().as_str(), error));
    let body = panel::body()
      .child(head)
      .with_children(parts.notice)
      .child(panel::chips_of(ctx, entry, badges, panel::used_by(ctx, &sheet.used_by)))
      .child(from_team(
        ctx,
        vec![
          used_by_row(ctx, &sheet.used_by, Some(ctx.t("team_hub.edit.usedBySub"))),
          format,
        ],
      ))
      .child(section)
      .child(on_this_computer(ctx, &Sheet::Instruction(sheet.clone())));
    panel::frame(entry, body, Some(parts.bar)).into()
  }
}

/// The instruction the fields make: `text` in the place of `base`'s, which keeps who gets it, when and where it is
/// added. A blank text, or one that names a variable the catalog does not have, makes none, and says why.
fn made(ctx: &Ctx, text: &str, base: &Instruction) -> Result<Instruction, Arc<str>> {
  if text.trim().is_empty() {
    return Err(ctx.t("team_hub.edit.textRequired"));
  }
  let text = PromptText::new(text).map_err(|error| message_text(ctx, &prompt_error(&error)))?;
  Ok(
    Instruction::new(
      text,
      base.targets().clone(),
      base.events().clone(),
      base.scope().clone(),
    )
    .expect("who gets it, when and where it is added are the published version's own, which it took"),
  )
}
