//! Saved/published editor header from bQqDC.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  node::{border::Border, dimension::Dimension},
};
use orchester_workspace::skills::{data::SkillEntry, editor::publish_reason};

use super::{
  super::{
    state::Panel,
    words,
    work::{self, Change},
  },
  Editor, HEADER_HEIGHT, Props,
};
use crate::ui::{
  breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
  button::{ActionButton, ButtonKind},
  message::message_text,
};
impl Editor {
  pub(super) fn header(
    &self,
    ctx: &mut Ctx,
    p: &Props,
    entry: Option<&SkillEntry>,
    title: &str,
    (dirty, busy): (bool, bool),
  ) -> Row {
    let back = p.page.view.panel.clone();
    let crumbs = breadcrumb(
      ctx,
      "skill-crumb",
      CrumbSeparator::Chevron,
      vec![
        Crumb::link(ctx.t("skills.title"), move || back.set(Panel::LIBRARY)),
        Crumb::here(title).mono(),
      ],
    );
    let action = self.primary_action(ctx, p, entry, (dirty, busy));
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .padding(SpacingSize::Md)
      .spacing(SpacingSize::Md)
      .align_items(lurq::layout::Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(crumbs)
      .child(
        Column::new()
          .width(Dimension::Px(1.0))
          .height(Dimension::Px(16.0))
          .background(PaletteColor::Border),
      )
      .child(
        crate::ui::icon::icon(
          if dirty {
            crate::ui::icon::Icon::CircleAlert
          } else if busy {
            crate::ui::icon::Icon::LoaderCircle
          } else if entry.is_some_and(|e| e.state.has_draft() && e.state.published().is_some()) {
            crate::ui::icon::Icon::CloudCheck
          } else if entry.is_some_and(|e| e.state.has_draft()) {
            crate::ui::icon::Icon::FilePenLine
          } else {
            crate::ui::icon::Icon::CircleCheck
          },
          crate::theme::IconSize::Base,
          if dirty && self.form_state(p).0.is_err() {
            PaletteColor::Danger
          } else {
            PaletteColor::TextSecondary
          },
        )
        .id("skill-save-icon"),
      )
      .child(
        Text::new(&self.status(ctx, p, entry, (dirty, busy)))
          .id("skill-save-status")
          .variant(TypographyStyle::Description)
          .color(if dirty && self.form_state(p).0.is_err() {
            PaletteColor::Danger
          } else {
            PaletteColor::TextSecondary
          }),
      )
      .child(Spacer::new().flex(1.0))
      .child(action)
  }
  fn primary_action(
    &self,
    ctx: &Ctx,
    p: &Props,
    entry: Option<&SkillEntry>,
    (dirty, busy): (bool, bool),
  ) -> ActionButton {
    if let Some(entry) = entry {
      let asking = self.asking.clone();
      let button = ActionButton::new(
        "skill-publish",
        ButtonKind::Primary,
        if entry.state.has_draft() {
          ctx.t_args("skills.publish", [("version", words::next(entry.state).to_string())])
        } else {
          ctx.t("skills.publishPlain")
        },
      )
      .icon(crate::ui::icon::Icon::Upload);
      if busy || dirty {
        button.unavailable_with_icon(ctx.t("skills.publishWaiting"))
      } else if let Some(reason) = publish_reason(entry) {
        button.unavailable_with_icon(message_text(ctx, &reason))
      } else {
        button.on_click(move || asking.set(true))
      }
    } else {
      let body = self.form_state(p).0;
      let editor = self.action.clone();
      let open = p.page.project.open.clone();
      let name = self.name.get();
      let submitted = self.submitted.clone();
      let choosing = self.choosing_file.clone();
      let button = ActionButton::new("skill-create", ButtonKind::Primary, ctx.t("skills.create"));
      if body.is_err() || busy || self.choosing_file.get() {
        button.disabled()
      } else {
        button.on_click(move || {
          if let Ok(body) = &body {
            if submitted.get_untracked() || choosing.get_untracked() {
              return;
            }
            submitted.set(true);
            editor.run((
              open.clone(),
              Change::Save {
                before: None,
                name: name.clone(),
                body: body.clone(),
              },
            ));
          }
        })
      }
    }
  }
  fn status(
    &self,
    ctx: &Ctx,
    p: &Props,
    entry: Option<&SkillEntry>,
    (dirty, busy): (bool, bool),
  ) -> std::sync::Arc<str> {
    if p.page.reloading {
      ctx.t("skills.reading")
    } else if busy && self.operation.get() == work::Operation::Publish {
      ctx.t("skills.publishing")
    } else if busy {
      ctx.t("skills.saving")
    } else if dirty {
      if self.form_state(p).0.is_err() {
        ctx.t("skills.fixFields")
      } else {
        ctx.t("skills.notSaved")
      }
    } else {
      entry.map_or_else(
        || ctx.t("skills.notSaved"),
        |e| {
          if matches!(
            e.state,
            orchester_workspace::agents::data::VersionState::StaleDraft { .. }
          ) {
            return words::version(ctx, e.state);
          }
          if e.state.has_draft()
            && let Some(version) = e.state.published()
          {
            ctx.t_args("skills.savedPublished", [("version", version.get().to_string())])
          } else if e.state.has_draft() {
            ctx.t("skills.savedUnpublished")
          } else {
            words::version(ctx, e.state)
          }
        },
      )
    }
  }
}
