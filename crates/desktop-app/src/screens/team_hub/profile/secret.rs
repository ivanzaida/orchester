//! *Add value* for a secret the profile needs, in a modal: the value is typed in a masked field and goes to the
//! keychain by the secret's name; it is never shown, logged or sent to the team (`docs/design/team-configuration.md`,
//! "What a computer fills in"). The design has no frame of its own for it.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, components::Column, core::Signal, node::Element};
use orchester_core::secret::SecretName;

use crate::ui::{
  button::{ActionButton, ButtonKind},
  dialog::{Dialog, Tone},
  field::FieldInput,
  icon::Icon,
};

/// What the dialog is about: the secret it takes a value for, the value typed, and whether it is open.
#[derive(Clone)]
pub(super) struct Entry {
  pub open: Signal<bool>,
  /// The secret's name, as text; empty while no secret is asked about.
  pub name: Signal<String>,
  pub value: Signal<String>,
}

impl Entry {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      name: ctx.signal(String::new()),
      value: ctx.signal(String::new()),
    }
  }

  /// Opens the dialog for `name`, with nothing typed.
  pub(super) fn ask(&self, name: &SecretName) {
    self.value.set(String::new());
    self.name.set(name.to_string());
    self.open.set(true);
  }

  /// Closes the dialog and forgets what was typed.
  pub(super) fn forget(&self) {
    self.open.set(false);
    self.value.set(String::new());
  }

  /// The dialog while it is open, `save` the action that takes the value for the name the dialog asks about.
  pub(super) fn dialog(
    &self,
    ctx: &mut Ctx,
    save: Arc<dyn Fn(SecretName, String) + Send + Sync>,
    pending: bool,
  ) -> Option<Element> {
    // The text is a secret's name that `ask` put there; empty (no name) while the dialog is not asked.
    let name = SecretName::new(&self.name.get()).ok()?;
    let typed = self.value.get();
    let field = FieldInput::new(
      "team-hub-secret-value",
      ctx.t("team_hub.secretDialog.field"),
      self.value.clone(),
    )
    .masked()
    .placeholder(ctx.t("team_hub.secretDialog.placeholder"))
    .render(ctx);
    let (entry, saving) = (self.clone(), name.clone());
    let button = ActionButton::new(
      "team-hub-secret-save",
      ButtonKind::Primary,
      ctx.t("team_hub.secretDialog.save"),
    )
    .icon(Icon::Check);
    let button = if pending {
      button.loading()
    } else if typed.trim().is_empty() {
      button.disabled()
    } else {
      button.on_click(move || {
        save(saving.clone(), entry.value.get());
        entry.forget();
      })
    };
    Some(
      Dialog::new(
        "team-hub-secret-dialog",
        Icon::KeyRound,
        ctx.t_args("team_hub.secretDialog.title", [("name", name.to_string())]),
        ctx.t("team_hub.secretDialog.cancel"),
      )
      .tone(Tone::Neutral)
      .what(ctx.t("team_hub.secretDialog.what"))
      .part(Column::new().child(field))
      .action(button)
      .render(ctx, &self.open),
    )
  }
}
