//! What an admin's edit puts in its panel besides the fields (`Agents · v2` A26–A31): *Unsaved changes* after
//! *Team · vN* while the fields differ from the version they are made on, the notice under the chips for what
//! the last Save was answered (A30 a newer version, a refusal) or that the team server cannot be reached while changes
//! wait (A31), and the fixed bar of Save and Cancel with what Save makes at its end.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{messages::Message, sharing::team_edit::EditRefused};

use super::{Body, Edit};
use crate::{
  screens::local_hub::save_bar::{self, Save},
  theme::HubText,
  ui::{
    chip::BadgeTone,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    pressable::Press,
  },
};

/// What the panel's fields make now, and how the panel stands.
pub(in crate::screens::team_hub) struct View<'a, B> {
  /// The body the fields make, or why they make none.
  pub made: Result<B, Arc<str>>,
  /// Why the team server cannot be reached now; Save waits for it.
  pub unreached: Option<&'a Message>,
  /// Who published the version the panel shows: what a newer version's notice names.
  pub published_by: Option<&'a str>,
}

/// What an edit adds to its panel.
pub(in crate::screens::team_hub) struct Parts {
  /// The fields differ from the version they are made on, or the last Save was refused: the bar must be Save's.
  pub holds: bool,
  /// *Unsaved changes*, after *Team · vN*.
  pub chip: Option<(Arc<str>, BadgeTone)>,
  /// Over the panel's sections.
  pub notice: Option<Element>,
  /// The fixed bar of Save and Cancel.
  pub bar: Element,
}

impl<B: Body> Edit<B> {
  /// What the edit adds to its panel, whose fields make `view.made`. `None` while the edit has no version to be made
  /// on.
  pub(in crate::screens::team_hub) fn parts(&self, ctx: &Ctx, view: View<'_, B>) -> Option<Parts> {
    let base = self.base()?;
    let changed = view.made.as_ref().map_or(true, |body| *body != base.body);
    let pending = self.save.state().get().is_pending();
    let save = if pending {
      Save::Saving
    } else if let Some(unreached) = view.unreached {
      Save::Waits(message_text(ctx, unreached))
    } else if !changed {
      Save::Unchanged
    } else {
      match view.made {
        Ok(body) => {
          let (edit, number, was) = (self.clone(), base.number, base.body.clone());
          Save::Ready(Press::run(move || edit.press_save(body.clone(), (number, &was))))
        }
        Err(why) => Save::Waits(why),
      }
    };
    let cancel = (changed && !pending).then(|| {
      let edit = self.clone();
      Press::run(move || edit.cancel())
    });
    let hint = hint(ctx, base.number, changed, view.unreached.is_some());
    let notice = match (self.refused.get(), view.unreached) {
      (Some(refused), _) => Some(refusal(ctx, &refused, base.number, view.published_by)),
      (None, Some(_)) if changed => Some(unreachable(ctx)),
      (None, _) => None,
    };
    let holds = changed || notice.is_some();
    Some(Parts {
      holds,
      chip: changed.then(|| (ctx.t("team_hub.edit.unsaved"), BadgeTone::Warning)),
      notice,
      bar: save_bar::bar(ctx, "team-hub-edit", (save, cancel), hint),
    })
  }
}

/// What the bar says at its end: what Save makes, that nothing differs, or that it needs the team server.
pub(in crate::screens::team_hub) fn hint(
  ctx: &Ctx,
  base: VersionNumber,
  changed: bool,
  unreached: bool,
) -> (String, PaletteColor) {
  let next = base.next().map_or_else(String::new, |next| next.to_string());
  let words = if unreached {
    ctx.t("team_hub.edit.needsServer")
  } else if changed {
    ctx.t_args("team_hub.edit.makes", [("version", next)])
  } else {
    ctx.t_args("team_hub.edit.makesUnchanged", [("version", next)])
  };
  (words.to_string(), PaletteColor::TextMuted)
}

/// A31: the team server cannot be reached while the fields hold changes, which stay.
fn unreachable(ctx: &Ctx) -> Element {
  Notice::new(
    "team-hub-edit-unreachable",
    NoticeTone::Warning,
    ctx.t("team_hub.edit.waiting.title"),
  )
  .icon(Icon::TriangleAlert)
  .title_style(HubText::NoticeTitle.role())
  .body(ctx.t("team_hub.edit.waiting.body"))
  .stacked()
  .into()
}

/// What the team server answered to the last Save: a newer version, which the fields show now (A30), or why it was
/// not taken, with what was typed kept.
fn refusal(ctx: &Ctx, refused: &EditRefused, base: VersionNumber, by: Option<&str>) -> Element {
  let next = base.next().map_or_else(String::new, |next| next.to_string());
  let (tone, icon, title, body) = match refused {
    EditRefused::Changed { latest } => {
      let version = latest.to_string();
      let title = match by {
        Some(name) => ctx.t_args(
          "team_hub.edit.newer.title",
          [("name", name.to_owned()), ("version", version.clone())],
        ),
        None => ctx.t_args("team_hub.edit.superseded.title", [("version", version.clone())]),
      };
      (
        NoticeTone::Warning,
        Icon::TriangleAlert,
        title,
        ctx.t_args("team_hub.edit.newer.body", [("version", version)]),
      )
    }
    EditRefused::Refused(refusal) => (
      NoticeTone::Danger,
      Icon::CircleX,
      ctx.t_args(
        if refusal.is_local() {
          "team_hub.edit.rejectedHere.title"
        } else {
          "team_hub.edit.rejected.title"
        },
        [("version", next)],
      ),
      ctx.t_args(
        "team_hub.edit.rejected.body",
        [("reason", message_text(ctx, &refusal.message()).to_string())],
      ),
    ),
    EditRefused::Unreachable(reason) => (
      NoticeTone::Danger,
      Icon::CircleX,
      ctx.t("team_hub.edit.unreachable.title"),
      message_text(ctx, reason),
    ),
    EditRefused::Failed(reason) => (
      NoticeTone::Danger,
      Icon::CircleX,
      ctx.t_args("team_hub.edit.failed.title", [("version", next)]),
      message_text(ctx, reason),
    ),
  };
  Notice::new("team-hub-edit-refused", tone, title)
    .icon(icon)
    .title_style(HubText::NoticeTitle.role())
    .body(body)
    .stacked()
    .into()
}
