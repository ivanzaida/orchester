//! *What automation may do* with its one built row, *Let automation change agent permissions* (`Form row`
//! `XaNIz` with a switch, under `Form section title` `De1nE`): an application setting that is off until a
//! person turns it on here. While it is off, the MCP server's setup tools refuse whatever would widen what
//! agents may do (`person_only`). It is read and saved off the UI thread; while either runs, the switch
//! cannot be flipped.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::Workspace,
};

use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, SettingsSpacing, SettingsText},
  ui::{
    field::field_error,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

const SWITCH_ID: &str = "settings-change-permissions";

/// No props: the row reads and saves the session's own setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoProps;

impl lurq::app::component::DevtoolsInspectable for NoProps {}

type SaveAction = FutureAction<bool, bool, Message>;

pub struct ChangePermissions {
  /// The setting as the person last chose it; `None` until they do.
  saved: Signal<Option<bool>>,
}

impl Component for ChangePermissions {
  type Props = NoProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      saved: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = Services::of(ctx).workspace;
    // Hooks in the same order on every render: the action, then the read, which follows each save.
    let save = save_action(ctx, &workspace);
    let state = save.state().get();
    let read = stored(ctx, &workspace, state.data);
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice once it saved; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    let problem = match (&read.error, &state.error) {
      (Some(reason), _) => Some(("settings.automation.changePermissions.unreadable", reason)),
      (None, Some(reason)) => Some(("settings.automation.changePermissions.notSaved", reason)),
      (None, None) => None,
    };
    let problem = problem.map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      field_error(
        "settings-change-permissions-error",
        &ctx.t_args(key, [("reason", &*reason)]),
      )
    });
    let title = ctx.t("settings.automation.changePermissions.title");
    let mut switch = Toggle::new(SWITCH_ID, ToggleKind::Switch, shown.unwrap_or(false), title.clone()).hide_label();
    // Until the stored value is known, and while it is read or saved, the switch stays as it is.
    if shown.is_some() && !fixed {
      let saved = self.saved.clone();
      switch = switch.on_change(move |chosen| {
        saved.set(Some(chosen));
        save.run(chosen);
      });
    }
    Column::new()
      .id("settings-automation")
      .width(Dimension::Pct(100.0))
      .child(section_title(&ctx.t("settings.automation.section")))
      .child(
        Row::new()
          .id("settings-change-permissions-row")
          .width(Dimension::Pct(100.0))
          .padding(Padding::vertical(SpacingSize::Md))
          .spacing(SpacingSize::Xl)
          .align_items(Alignment::Center)
          .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .child(
            Column::new()
              .flex(1.0)
              .spacing(ExtraSpacing::Fine.role())
              .child(Text::new(&title).variant(ExtraTypography::Strong.role()))
              .child(
                Text::new(&ctx.t("settings.automation.changePermissions.description"))
                  .variant(ExtraTypography::Prose.role())
                  .width(Dimension::Pct(100.0)),
              )
              .with_children(problem),
          )
          .child(switch),
      )
  }
}

/// A form section's title over its line.
fn section_title(title: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_bottom(SettingsSpacing::SectionTitle.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(title).variant(SettingsText::SectionTitle.role()))
}

/// Saves the setting the person chose.
fn save_action(ctx: &mut Ctx, workspace: &Workspace) -> SaveAction {
  let workspace = workspace.clone();
  ctx.future_action(move |allowed: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_automation_changes_permissions(allowed))
        .await?
        .map(|()| allowed)
        .map_err(|error| {
          tracing::error!(%error, "Let automation change agent permissions was not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// The setting as stored, read when the row shows and again after each save (`saved`).
fn stored(ctx: &mut Ctx, workspace: &Workspace, saved: Option<bool>) -> FutureState<bool, Message> {
  let workspace = workspace.clone();
  let read = ctx.future(saved, move |_| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.automation_changes_permissions())
        .await?
        .map_err(|error| {
          tracing::error!(%error, "Let automation change agent permissions cannot be read");
          messages::store_error(&error)
        })
    }
  });
  read.state().get()
}
