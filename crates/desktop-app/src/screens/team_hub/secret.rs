//! *Add value* for a secret the profile needs, in a modal: the value is typed in a masked field and goes to the
//! keychain by the secret's name; it is never shown, logged or sent to the team (`docs/design/team-configuration.md`,
//! "What a computer fills in"). The design has no frame of its own for it.
//!
//! The typed text is held the way the Secrets page holds it (`screens/secrets/form.rs`): in a signal made outside
//! `Ctx`, so DevTools never records it, and overwritten when the dialog closes (*Cancel*, Esc, the scrim, a saved
//! value) and when the panel goes. It becomes a [`SecretValue`] when *Save value* is pressed, and the dialog stays
//! open until the save has answered: a failure shows in the dialog and the typed text is still there.

use std::sync::Arc;

use lurq::{
  app::ctx::{Ctx, FutureAction},
  components::Column,
  core::Signal,
  node::Element,
};
use orchester_core::secret::{SecretName, SecretValue};
use orchester_workspace::messages::Message;
use zeroize::Zeroize;

use crate::{
  screens::settings::orchester::PersonOnly,
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::{Dialog, Tone},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
  },
};

/// The id the person-only check of the dialog's *Save* is made under.
pub(super) const SAVE: &str = "team-hub-secret-save";

/// What the dialog's *Save value* hands over: the secret's name and its value.
pub(super) type Save = Arc<dyn Fn(SecretName, SecretValue) + Send + Sync>;

/// What the dialog is about: the secret it takes a value for, the value typed, and whether it is open.
#[derive(Clone)]
pub(super) struct Entry {
  pub open: Signal<bool>,
  /// Why the value was not kept, shown in the dialog.
  pub error: Signal<Option<Message>>,
  /// The secret's name, as text; empty while no secret is asked about.
  name: Signal<String>,
  /// Deliberately not a `Ctx` signal: no value in DevTools or its history.
  value: Signal<String>,
}

impl Entry {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let entry = Self {
      open: ctx.signal(false),
      error: ctx.signal(None),
      name: ctx.signal(String::new()),
      value: Signal::new(String::new()),
    };
    let value = entry.value.clone();
    ctx.watch(&entry.open, move |open| {
      if !open {
        value.update(Zeroize::zeroize);
      }
    });
    entry
  }

  /// Opens the dialog for `name`, with nothing typed.
  pub(super) fn ask(&self, name: &SecretName) {
    self.wipe();
    self.error.set(None);
    self.name.set(name.to_string());
    self.open.set(true);
  }

  /// A saved value closes the dialog; a failure shows in it, with the typed text kept.
  pub(super) fn follow_save(&self, ctx: &mut Ctx, add_value: &FutureAction<(SecretName, SecretValue), (), Message>) {
    let (open, error) = (self.open.clone(), self.error.clone());
    ctx.watch(&add_value.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if state.is_fulfilled() {
        open.set(false);
      } else if let Some(failed) = &state.error {
        error.set(Some(failed.clone()));
      }
    });
  }

  /// What the dialog's *Save value* does, a person's alone: the value goes on as the person's.
  pub(super) fn save_by(add_value: &FutureAction<(SecretName, SecretValue), (), Message>, person: &PersonOnly) -> Save {
    let (action, person) = (add_value.clone(), person.clone());
    Arc::new(move |name, value| {
      if person.allows(SAVE) {
        action.run((name, value));
      }
    })
  }

  /// Overwrites what was typed.
  pub(super) fn wipe(&self) {
    self.value.update(Zeroize::zeroize);
  }

  /// The dialog while it is open. `save` takes the value for the name the dialog asks about; `saving` while the save
  /// is under way; `under` is what the body says after the field.
  pub(super) fn dialog(&self, ctx: &mut Ctx, save: Save, saving: bool, under: Option<Element>) -> Option<Element> {
    // The text is a secret's name that `ask` put there; empty (no name) while the dialog is not asked.
    let name = SecretName::new(&self.name.get()).ok()?;
    let field = FieldInput::new(
      "team-hub-secret-value",
      ctx.t("team_hub.secretDialog.field"),
      self.value.clone(),
    )
    .masked()
    .placeholder(ctx.t("team_hub.secretDialog.placeholder"))
    .render(ctx);
    let failed = self
      .error
      .get()
      .map(|error| Element::from(field_error("team-hub-secret-error", &message_text(ctx, &error))));
    let button = ActionButton::new(
      "team-hub-secret-save",
      ButtonKind::Primary,
      ctx.t("team_hub.secretDialog.save"),
    )
    .icon(Icon::Check);
    let button = if saving {
      button.loading()
    } else if self.value.with(|typed| typed.trim().is_empty()) {
      button.disabled()
    } else {
      let (entry, asked) = (self.clone(), name.clone());
      button.on_click(move || entry.hand_over(&asked, &save))
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
      .under(under.into_iter().chain(failed))
      .action(button)
      .render(ctx, &self.open),
    )
  }

  /// Hands the typed text to `save` as a value, which stays here too until the save has answered.
  fn hand_over(&self, name: &SecretName, save: &Save) {
    // The copy `SecretValue` takes is the one that goes on; a blank text is refused and wiped by it.
    match self.value.with(|typed| SecretValue::new(typed.clone())) {
      Ok(value) => {
        self.error.set(None);
        save(name.clone(), value);
      }
      Err(_) => self.error.set(Some(Message::new("teamFillIns.errors.valueBlank"))),
    }
  }
}
