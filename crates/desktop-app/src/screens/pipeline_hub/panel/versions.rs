//! The panel while it shows a published version read-only ([`super::super::version_view`]), opened from *Versions* in
//! the head: in the place of the pipeline's body the version's notice, steps and settings, with no Save bar, which
//! belongs to the edit that waits behind it. *Restore* puts the version into that edit; while the edit holds changes
//! it asks first, in a dialog over the page, because they would be replaced. *Back* returns to the edit.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Column, node::Element};

use super::{Hosted, PipelinePanel, Shown, body};
use crate::{
  screens::pipeline_hub::{
    edit::Edit,
    version_view::{self, Parts, Restore, Viewed},
  },
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    icon::Icon,
  },
};

impl PipelinePanel {
  /// The body for `viewed`: the head, which names the version, and the version's notice, steps and settings.
  pub(super) fn version_body(&self, ctx: &mut Ctx, shown: &Shown<'_>, hosted: Hosted, viewed: &Viewed) -> Column {
    let head = self.head(ctx, shown, hosted);
    let held = version_view::is_held(shown.current, viewed);
    let restore = self.restore_now(shown.edit, viewed, (shown.changed, held, shown.locked));
    body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .with_children(version_view::parts(
        ctx,
        &Parts {
          project: shown.project,
          entry: shown.entry,
          assignables: shown.assignables,
          viewed,
          restore,
          changed: shown.changed,
          back: self.back_to_editing(),
        },
      ))
  }

  /// Closes the version that is shown, and the question about it.
  pub(super) fn close_version(&self) {
    if self.viewing.get_untracked().is_some() {
      self.restoring.set(false);
      self.viewing.set(None);
    }
  }

  /// *Back*: the panel shows the edit, or what is stored, again.
  fn back_to_editing(&self) -> Arc<dyn Fn() + Send + Sync> {
    let (viewing, restoring) = (self.viewing.clone(), self.restoring.clone());
    Arc::new(move || {
      restoring.set(false);
      viewing.set(None);
    })
  }

  /// What *Restore* does for `viewed`: nothing while a save is under way or asks, or while the panel already holds the
  /// version; else it asks first when the edit has changes it would replace, and otherwise puts the version in.
  fn restore_now(&self, edit: &Edit, viewed: &Viewed, (changed, held, locked): (bool, bool, bool)) -> Restore {
    if locked {
      return Restore::Busy;
    }
    if held {
      return Restore::Same;
    }
    if changed {
      let restoring = self.restoring.clone();
      return Restore::Ready(Arc::new(move || restoring.set(true)));
    }
    Restore::Ready(self.put_in(edit, viewed))
  }

  /// Puts `viewed` into the edit and shows the edit: the description's box follows it, as every change to the edit
  /// does.
  fn put_in(&self, edit: &Edit, viewed: &Viewed) -> Arc<dyn Fn() + Send + Sync> {
    let (edit, viewed, description) = (edit.clone(), viewed.clone(), self.description.clone());
    let (viewing, restoring) = (self.viewing.clone(), self.restoring.clone());
    Arc::new(move || {
      edit.restore(&viewed.body);
      description.set(viewed.body.description().to_owned());
      restoring.set(false);
      viewing.set(None);
    })
  }

  /// The question before *Restore* drops the changes the edit holds, in a dialog over the page.
  pub(super) fn replace_question(&self, ctx: &Ctx, edit: &Edit, viewed: &Viewed) -> Option<Element> {
    if !self.restoring.get() {
      return None;
    }
    let version = viewed.number.to_string();
    let restore = self.put_in(edit, viewed);
    Some(
      Dialog::new(
        "pipeline-hub-version-replace",
        Icon::Undo2,
        ctx.t_args("pipeline_hub.versions.replaceTitle", [("version", version.clone())]),
        ctx.t("pipeline_hub.leave.keep"),
      )
      .tone(Tone::Neutral)
      .what(ctx.t_args("pipeline_hub.versions.replaceWhat", [("version", version.clone())]))
      .action(
        ActionButton::new(
          "pipeline-hub-version-replace-confirm",
          ButtonKind::Primary,
          ctx.t_args("pipeline_hub.versions.restore", [("version", version)]),
        )
        .icon(Icon::Undo2)
        .on_click(move || restore()),
      )
      .render(ctx, &self.restoring),
    )
  }
}
