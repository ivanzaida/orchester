//! Approval (`Approval` `HPtH5`): that the first run that starts the service asks a person, and that a change to its
//! program, arguments, variables or seed asks again, with where the service stands at the row's end as a badge
//! (`Badge · Not approved yet` `s51QQ`): not approved yet, approved for every run, or changed since it was approved.
//! It follows the fields as they are now, so a change asks again before it is saved.

use lurq::{app::ctx::Ctx, node::Element};
use orchester_workspace::agent_testing::editor::ApprovalState;

use super::{
  super::super::kit::{Group, SettingRow},
  View,
};
use crate::ui::chip::{BadgeTone, badge};

/// The Approval group; none for a new service, which no run can start yet.
pub(super) fn approval_group(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let service = view.built.as_ref().ok().or_else(|| view.base())?;
  view.base()?;
  let (tone, key) = match view.stored.approval(service) {
    ApprovalState::NotApproved => (
      BadgeTone::Neutral,
      "project_settings.services.editor.approval.notApproved",
    ),
    ApprovalState::Approved => (BadgeTone::Success, "project_settings.services.editor.approval.approved"),
    ApprovalState::Changed => (BadgeTone::Warning, "project_settings.services.editor.approval.changed"),
  };
  let row = SettingRow::new(
    "settings-service-approval",
    ctx.t("project_settings.services.editor.approval.label"),
  )
  .hint(ctx.t("project_settings.services.editor.approval.hint"))
  .control(badge("settings-service-approval-state", tone, &ctx.t(key)))
  .render(ctx);
  Some(
    Group::new(
      "settings-service-approval-group",
      ctx.t("project_settings.services.editor.approval.title"),
    )
    .row(row)
    .into(),
  )
}
