//! *What automation may do* with its one built row, *Let automation change agent permissions* (`Form row`
//! `XaNIz` with a switch, under `Form section title` `De1nE`): an application setting that is off until a
//! person turns it on here. While it is off, the MCP server's setup tools refuse whatever would widen what
//! agents may do (`person_only`). It is read and saved off the UI thread; while either runs, the switch
//! cannot be flipped. While the row is mounted, the app's MCP server refuses window input
//! (`mcp::input_gate`), so only a person flips it.

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
  mcp::input_gate::{InputGate, Protected},
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
  /// A change the row did not take because an MCP client's input brought it.
  blocked: Signal<bool>,
  gate: InputGate,
  /// While the row is mounted, the MCP server refuses window input.
  _protected: Protected,
}

impl Component for ChangePermissions {
  type Props = NoProps;

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    Self {
      saved: ctx.signal(None),
      blocked: ctx.signal(false),
      _protected: gate.protect(),
      gate,
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
    let problem = problem(ctx, &read.error, &state.error);
    // The row takes no change before it lived through one pass after mounting: an input meant for what
    // the window showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    let title = ctx.t("settings.automation.changePermissions.title");
    let switch = Toggle::new(SWITCH_ID, ToggleKind::Switch, shown.unwrap_or(false), title.clone()).hide_label();
    // Until the stored value is known, and while it is read or saved, the switch stays as it is.
    let switch = if shown.is_some() && !fixed && armed {
      self.listen(switch, save)
    } else {
      switch
    };
    let blocked = self.blocked.get().then(|| {
      Text::new(&ctx.t("settings.automation.changePermissions.blocked"))
        .id("settings-change-permissions-blocked")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
    });
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
              .with_children(problem)
              .with_children(blocked),
          )
          .child(switch),
      )
  }
}

impl ChangePermissions {
  /// `switch`, saving what a person chooses. Turning it on while an MCP client's input is applied is not
  /// taken, and the row says so; turning it off narrows what automation may do and is always taken.
  fn listen(&self, switch: Toggle, save: SaveAction) -> Toggle {
    let (saved, blocked, gate) = (self.saved.clone(), self.blocked.clone(), self.gate.clone());
    switch.on_change(move |chosen| {
      if chosen && gate.input_in_flight() {
        tracing::warn!("an MCP client's window input reached Let automation change agent permissions; not taken");
        blocked.set(true);
        return;
      }
      blocked.set(false);
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }
}

/// Why the setting cannot be read or was not saved, under the row's description.
fn problem(ctx: &Ctx, read: &Option<Message>, save: &Option<Message>) -> Option<Row> {
  let (key, reason) = match (read, save) {
    (Some(reason), _) => ("settings.automation.changePermissions.unreadable", reason),
    (None, Some(reason)) => ("settings.automation.changePermissions.notSaved", reason),
    (None, None) => return None,
  };
  let reason = message_text(ctx, reason);
  Some(field_error(
    "settings-change-permissions-error",
    &ctx.t_args(key, [("reason", &*reason)]),
  ))
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
