//! What an API provider's row does in Settings › Accounts (`Agents on accounts` A1 `aOR4w`; B1 `xN1Jy` in part):
//! *Replace key…* takes a new key for it, typed once into a question over the window and kept by the app in place of
//! the one it had, and *Remove* takes the account and its key away, after a question. A project's connection on a
//! removed account stays as a record of what ran on it, and says its key is missing.
//!
//! Not here yet: renaming an account, another base URL or other models for it, and testing it.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{account::ApiAccount, id::ApiAccountId, secret::SecretValue};
use orchester_workspace::{background, messages::Message};
use zeroize::Zeroize;

use super::words::{t, ta};
use crate::{
  services::Services,
  theme::{IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::Dialog,
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    message::message_text,
    setting_rows::{SettingRow, row_tile, status},
  },
};

/// What the rows of the API providers do, and the question one of them asks.
#[derive(Clone)]
pub(super) struct ApiRows {
  pub remove: FutureAction<ApiAccountId, (), Message>,
  pub replace: FutureAction<(ApiAccountId, SecretValue), (), Message>,
  /// The account a question is about, by its id as text; empty while none is asked.
  asked: Signal<String>,
  /// Whether *Remove …?* shows, and whether *Replace the key of …* does.
  removing: Signal<bool>,
  replacing: Signal<bool>,
  /// The new key as typed. Deliberately not a ctx signal: no value in DevTools or its history.
  key: Signal<String>,
  key_revision: Signal<u64>,
}

impl ApiRows {
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace.clone(), services.vault.clone());
    let remove = ctx.future_action(move |id: ApiAccountId| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        background::run(move || {
          workspace
            .remove_api_account(vault.get(), id)
            .map_err(|error| error.message())
        })
        .await?
      }
    });
    let (workspace, vault) = (services.workspace, services.vault);
    let replace = ctx.future_action(move |(id, key): (ApiAccountId, SecretValue)| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        background::run(move || {
          workspace
            .replace_api_account_key(vault.get(), id, &key)
            .map(|_| ())
            .map_err(|error| error.message())
        })
        .await?
      }
    });
    Self {
      remove,
      replace,
      asked: ctx.signal(String::new()),
      removing: ctx.signal(false),
      replacing: ctx.signal(false),
      key: Signal::new(String::new()),
      key_revision: ctx.signal(0),
    }
  }

  /// One API account's row: a cloud mark, its name over `line`, that its key is kept, *Replace key…* and *Remove*.
  pub fn row(&self, ctx: &mut Ctx, account: &ApiAccount, line: String) -> Element {
    let id = format!("api-row-{}", account.id());
    let busy = self.remove.state().get().is_pending() || self.replace.state().get().is_pending();
    let ask = |open: &Signal<bool>| {
      let (asked, open, key, account) = (self.asked.clone(), open.clone(), self.key.clone(), account.id());
      move || {
        key.update(Zeroize::zeroize);
        asked.set(account.to_string());
        open.set(true);
      }
    };
    let button = |name: &str, key: &str, glyph: Icon, open: &Signal<bool>| {
      let button = ActionButton::new(format!("{id}-{name}"), ButtonKind::Ghost, t(ctx, key))
        .icon(glyph)
        .small();
      if busy {
        button.disabled()
      } else {
        button.on_click(ask(open))
      }
    };
    let end = Row::new()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(status(
        &format!("{id}-state"),
        (PaletteColor::Success, PaletteColor::TextSecondary),
        &t(ctx, "accounts.api.keySet"),
      ))
      .child(button(
        "replace",
        "accounts.api.replaceKey",
        Icon::KeyRound,
        &self.replacing,
      ))
      .child(button("remove", "accounts.api.remove", Icon::Trash2, &self.removing));
    SettingRow::new(id, account.name().as_str().to_owned())
      .lead(row_tile(icon(Icon::Cloud, IconSize::Md, PaletteColor::TextSecondary)))
      .hint(line)
      .control(end)
      .render(ctx)
  }

  /// The question that shows over the window for the account asked about: *Remove …?* or *Replace the key of …*, with
  /// why the last answer did nothing; nothing while none is asked.
  pub fn questions(&self, ctx: &mut Ctx, accounts: &[ApiAccount]) -> Vec<Element> {
    let asked = self.asked.get();
    let Some(account) = accounts.iter().find(|account| account.id().to_string() == asked) else {
      return Vec::new();
    };
    vec![self.remove_question(ctx, account), self.replace_question(ctx, account)]
  }

  fn remove_question(&self, ctx: &Ctx, account: &ApiAccount) -> Element {
    let (remove, close, id) = (self.remove.clone(), self.removing.clone(), account.id());
    let confirm =
      ActionButton::new("api-remove-confirm", ButtonKind::Danger, t(ctx, "accounts.api.remove")).on_click(move || {
        close.set(false);
        remove.run(id);
      });
    Dialog::new(
      "api-remove-dialog",
      Icon::Trash2,
      ta(
        ctx,
        "accounts.api.removeTitle",
        [("name", account.name().as_str().to_owned())],
      ),
      t(ctx, "accounts.add.cancel"),
    )
    .part(Text::new(&t(ctx, "accounts.api.removeBody")).variant(ProjectSettingsText::Hint.role()))
    .action(confirm)
    .render(ctx, &self.removing)
  }

  fn replace_question(&self, ctx: &mut Ctx, account: &ApiAccount) -> Element {
    // Read so that a typed key draws the question again: the value itself is no ctx signal.
    self.key_revision.get();
    let typed = self.key.with(|value| SecretValue::new(value.clone()).ok());
    let (replace, close, key, id) = (
      self.replace.clone(),
      self.replacing.clone(),
      self.key.clone(),
      account.id(),
    );
    let confirm = ActionButton::new(
      "api-replace-confirm",
      ButtonKind::Primary,
      t(ctx, "accounts.api.replaceConfirm"),
    );
    let confirm = if typed.is_some() {
      confirm.on_click(move || {
        // Read again at the press, and wiped from the field once it is on its way to the keychain.
        if let Some(value) = key.with(|value| SecretValue::new(value.clone()).ok()) {
          key.update(Zeroize::zeroize);
          close.set(false);
          replace.run((id, value));
        }
      })
    } else {
      confirm.disabled()
    };
    let revision = self.key_revision.clone();
    let label = t(ctx, "accounts.api.newKey");
    let field = FieldInput::new("api-replace-key", label, self.key.clone())
      .masked()
      .on_typed(move |_| revision.update(|value| *value += 1))
      .render(ctx);
    let state = self.replace.state().get();
    let pending = state.is_pending();
    let failed = state
      .error
      .filter(|_| !pending)
      .map(|error| field_error("api-replace-error", &message_text(ctx, &error)));
    Dialog::new(
      "api-replace-dialog",
      Icon::KeyRound,
      ta(
        ctx,
        "accounts.api.replaceTitle",
        [("name", account.name().as_str().to_owned())],
      ),
      t(ctx, "accounts.add.cancel"),
    )
    .part(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Text::new(&t(ctx, "accounts.api.replaceBody")).variant(ProjectSettingsText::Hint.role()))
        .child(field)
        .with_children(failed),
    )
    .action(confirm)
    .render(ctx, &self.replacing)
  }
}
