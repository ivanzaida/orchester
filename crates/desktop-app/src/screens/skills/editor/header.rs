//! The top of a skill's page (`03b Skill` `QbDWX`): the crumb back to Skills, the head with the skill's name, scope
//! and origin and the … menu with Remove, and the draft bar — what the skill is now (*Draft written by automation ·
//! Published v3*), what publishing does, Discard draft and Publish (Create on a new skill).
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::{
  agents::data::VersionState,
  skills::{data::SkillEntry, editor::publish_reason},
};

use super::{
  super::{
    words,
    work::{self, Change},
  },
  Ask, Editor, Props,
};
use crate::{
  screens::hub::{
    draft_bar::{DraftBar, Standing},
    page_head::{self, More},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

impl Editor {
  pub(super) fn crumb(&self, ctx: &Ctx, p: &Props, entry: Option<&SkillEntry>) -> Element {
    let back = p.page.view.panel.clone();
    let new = ctx.t("skills.new");
    page_head::crumb(
      "skill-crumb",
      (&ctx.t("hub.tabs.skills"), back.to(back.library())),
      entry.is_none().then_some(&*new),
    )
  }

  pub(super) fn page_head(&self, ctx: &mut Ctx, p: &Props, entry: Option<&SkillEntry>, title: &str) -> Element {
    let sub = entry.map_or_else(
      || ctx.t("skills.newSub"),
      |e| {
        let scope = words::scope(ctx, &p.page.data, e.body.scope());
        match &e.origin {
          Some(origin) => ctx.t_args(
            "skills.pageSubImported",
            [
              ("scope", scope.to_string()),
              ("source", origin.source.as_str().to_owned()),
            ],
          ),
          None => scope,
        }
      },
    );
    let more = entry.map(|_| {
      let (ask, open) = (self.ask.clone(), self.more.clone());
      More {
        open: self.more.clone(),
        anchor: self.anchor.clone(),
        remove: Arc::new(move || {
          open.set(false);
          ask.set(Ask::Remove);
        }),
      }
    });
    page_head::head(ctx, "skill", (Icon::GraduationCap, title, &sub), more)
  }

  pub(super) fn bar(&self, ctx: &Ctx, p: &Props, entry: Option<&SkillEntry>, (dirty, busy): (bool, bool)) -> Element {
    let invalid = dirty && self.form_state(p).0.is_err();
    let standing = match entry.map(|e| e.state) {
      _ if invalid => Standing::Attention,
      Some(VersionState::Published(_)) if !dirty && !busy => Standing::Published,
      Some(VersionState::StaleDraft { .. }) => Standing::Attention,
      _ => Standing::Draft,
    };
    let effect = entry.and_then(|e| e.state.published()).map_or_else(
      || ctx.t("skills.publishFirst"),
      |v| ctx.t_args("skills.publishEffect", [("version", v.get().to_string())]),
    );
    // `skill-save`: its status and glyph keep the ids the skill page gave its save state (`skill-save-status`).
    let mut bar = DraftBar::new("skill-save", standing, self.status(ctx, p, entry, (dirty, busy))).sub(effect);
    let discards = entry.filter(|e| e.state.has_draft() && e.state.published().is_some());
    if discards.is_some() {
      let ask = self.ask.clone();
      let discard =
        ActionButton::new("skill-discard", ButtonKind::Ghost, ctx.t("hub.discard.action")).icon(Icon::Undo2);
      bar = bar.action(if busy || dirty {
        discard.disabled()
      } else {
        discard.on_click(move || ask.set(Ask::Discard))
      });
    }
    bar.action(self.primary_action(ctx, p, entry, (dirty, busy))).into()
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
      .icon(Icon::Upload);
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
      let authorship = self.authorship.clone();
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
                by_automation: authorship.saving(),
              },
            ));
          }
        })
      }
    }
  }

  fn status(&self, ctx: &Ctx, p: &Props, entry: Option<&SkillEntry>, (dirty, busy): (bool, bool)) -> Arc<str> {
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
      let automated = automated(p, entry);
      entry.map_or_else(
        || ctx.t("skills.newNote"),
        |e| {
          if matches!(e.state, VersionState::StaleDraft { .. }) {
            return words::version(ctx, e.state);
          }
          match (e.state.has_draft(), e.state.published(), automated) {
            (true, Some(version), true) => ctx.t_args(
              "skills.automationSavedPublished",
              [("version", version.get().to_string())],
            ),
            (true, None, true) => ctx.t("skills.automationSavedUnpublished"),
            (true, Some(version), false) => {
              ctx.t_args("skills.savedPublished", [("version", version.get().to_string())])
            }
            (true, None, false) => ctx.t("skills.savedUnpublished"),
            (false, ..) => words::version(ctx, e.state),
          }
        },
      )
    }
  }
}

/// Whether `entry`'s draft, current and saved, is the one automation saved over Orchester's MCP server (ORC-79, as
/// ORC-30 marks an instruction's): who wrote it shows before a person publishes it.
fn automated(p: &Props, entry: Option<&SkillEntry>) -> bool {
  entry.is_some_and(|e| matches!(e.state, VersionState::Draft { .. }) && p.page.data.listed.by_automation(e))
}
