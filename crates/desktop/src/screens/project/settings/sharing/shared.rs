//! A shared project's Sharing (10.13): the server project its task list lives in, the discovery
//! file, the members and *Make local*.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_api::MemberRole;

use super::{
  Actions, Facts, SharingDialog, SharingTab, labels,
  local::{CONTROL_HEIGHT, VALUE_WIDTH},
};
use crate::{
  screens::project::settings::general::{readback, sized_setting_row},
  sharing::project::{discovery::Discovery, state::ProjectSharing},
  theme::ExtraTypography,
  ui::{button::ActionButton, field::field_error, icon::Icon, message_text, status_label::status_label},
  workspace::OpenProject,
};

impl SharingTab {
  pub(super) fn shared(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    sharing: &ProjectSharing,
    facts: &Facts,
    actions: &Actions,
  ) -> Vec<Element> {
    vec![
      task_list_row(ctx, open, sharing),
      self.discovery_row(ctx, open, sharing, facts, actions),
      self.members(ctx, sharing, actions),
      self.make_local_row(ctx, open, sharing, actions),
    ]
  }

  /// *Discovery file* (`O7C7DV`): `orchester.toml` as the repository has it, and *Write
  /// orchester.toml* while it has none.
  fn discovery_row(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    sharing: &ProjectSharing,
    facts: &Facts,
    actions: &Actions,
  ) -> Element {
    let file = &facts.discovery;
    let ours = matches!(&file.discovery, Discovery::Server(address)
      if sharing.server.as_ref().is_some_and(|(_, ours)| ours == address));
    let mut value = Vec::new();
    match (&file.discovery, ours) {
      (Discovery::Server(_), true) => {
        let written = file.written_at.map_or_else(
          || ctx.t("sharing.discoveryFile.present"),
          |at| ctx.t_args("sharing.discoveryFile.written", [("date", &*labels::day(ctx, at))]),
        );
        let committed = match file.committed {
          Some(false) => ctx.t("sharing.discoveryFile.notCommitted"),
          Some(true) => ctx.t("sharing.discoveryFile.committed"),
          None => ctx.t("sharing.discoveryFile.unknown"),
        };
        value.push(
          Text::new(&written)
            .id("sharing-discovery-file")
            .variant(ExtraTypography::MonoField.role())
            .into(),
        );
        value.push(readback(&committed));
      }
      (Discovery::Server(_) | Discovery::Unreadable, _) => {
        value.push(readback(&ctx.t("sharing.discoveryFile.other")));
      }
      (Discovery::Absent, _) => {
        value.push(readback(&ctx.t("sharing.discoveryFile.absent")));
        let state = actions.discovery.state().get();
        let action = actions.discovery.clone();
        let button = ActionButton::new(ctx.t("sharing.discoveryFile.write"))
          .icon(Icon::FileSymlink)
          .id("sharing-write-discovery")
          .in_form()
          .height(Dimension::Px(CONTROL_HEIGHT));
        let button = if state.is_pending() || sharing.server.is_none() {
          button.on_click(|| {}).enabled(false)
        } else {
          button.on_click(move || action.run(()))
        };
        value.push(button.render(ctx));
        if let Some(error) = state.error.clone().filter(|_| state.is_rejected()) {
          value.push(field_error(&message_text(ctx, &error)));
        }
      }
    }
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("sharing.discoveryFile.name"),
      &ctx.t_args(
        "sharing.discoveryFile.detail",
        [("project", open.project.name().as_str())],
      ),
      value,
    )
  }

  /// *Make local* (`tNgaa`).
  fn make_local_row(&self, ctx: &Ctx, open: &OpenProject, sharing: &ProjectSharing, actions: &Actions) -> Element {
    let dialog = self.dialog.clone();
    let button = ActionButton::new(ctx.t("sharing.makeLocal.action"))
      .icon(Icon::HardDriveDownload)
      .id("sharing-make-local")
      .in_form()
      .height(Dimension::Px(CONTROL_HEIGHT));
    let button = if actions.make_local.state().get().is_pending() {
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || dialog.set(SharingDialog::MakeLocal))
    };
    sized_setting_row(
      VALUE_WIDTH,
      &ctx.t("sharing.makeLocal.name"),
      &ctx.t_args(
        "sharing.makeLocal.detail",
        [
          ("project", open.project.name().as_str()),
          ("key", sharing.link.key.as_str()),
        ],
      ),
      vec![button.render(ctx)],
    )
  }
}

/// *Task list* (`dX7Vp`): *Shared on* the server, with its key, since when, the person's role and
/// when the copy here last matched it.
fn task_list_row(ctx: &Ctx, open: &OpenProject, sharing: &ProjectSharing) -> Element {
  let server = sharing.server_name();
  let role = match (sharing.link.role, sharing.admin) {
    (Some(MemberRole::Owner), _) => "sharing.taskList.sharedReadback.owner",
    (Some(MemberRole::Member), _) => "sharing.taskList.sharedReadback.member",
    (None, _) => "sharing.taskList.sharedReadback.admin",
  };
  let since = labels::day(ctx, sharing.link.joined_at);
  let synced = labels::time(sharing.link.last_synced_at);
  let back: Arc<str> = ctx.t_args(
    role,
    [
      ("key", sharing.link.key.as_str()),
      ("date", &*since),
      ("time", synced.as_str()),
    ],
  );
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("sharing.taskList.name"),
    &ctx.t_args(
      "sharing.taskList.shared",
      [("project", open.project.name().as_str()), ("server", server)],
    ),
    vec![
      Row::new()
        .id("sharing-task-list")
        .child(status_label(
          Icon::Cloud,
          &ctx.t_args("sharing.taskList.sharedLabel", [("server", server)]),
        ))
        .into(),
      Text::new(&back)
        .id("sharing-task-list-readback")
        .variant(ExtraTypography::Footnote.role())
        .color(lurq::app::theme::PaletteColor::TextMuted)
        .into(),
    ],
  )
}
