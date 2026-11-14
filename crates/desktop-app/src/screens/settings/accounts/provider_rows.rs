//! An API provider's row in the list of accounts (`Agents on accounts` A1 `aOR4w`): a cloud mark, its name over
//! its line, how its key stands and a chevron; a press opens its page ([`super::provider`]). How it stands is said
//! the same way at the head of that page: its key is not in this computer's keychain, what the last test of a
//! model API Orchester calls found, or that its key is set.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{account::ApiAccount, connection::CheckResult, id::ApiAccountId};

use super::words::t;
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::IconSize,
  ui::{
    icon::{Icon, icon},
    setting_rows::{SettingRow, opens_mark, row_tile, status},
  },
};

/// Whether this computer's keychain holds each provider's key, and where a row's press goes.
pub(super) struct Rows {
  pub navigation: Navigation,
  /// The providers whose key the keychain does not hold; `None` until that is read.
  pub missing: Option<Arc<Vec<ApiAccountId>>>,
}

impl Rows {
  /// One provider's row: a cloud mark, its name over `line`, how its key stands and a chevron; a press opens its
  /// page.
  pub fn row(&self, ctx: &mut Ctx, account: &ApiAccount, line: String) -> Element {
    let id = format!("api-row-{}", account.id());
    let end = Row::new()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(self.state(ctx, &format!("{id}-state"), account))
      .child(opens_mark());
    SettingRow::new(id, account.name().as_str().to_owned())
      .lead(row_tile(icon(Icon::Cloud, IconSize::Md, PaletteColor::TextSecondary)))
      .hint(line)
      .control(end)
      .opens(self.navigation.to(Route::Accounts(AccountsView::of_provider(
        account.id(),
        AccountsPanel::Account,
      ))))
      .render(ctx)
  }

  /// How the provider stands: its key is missing here, what its last test found, or that its key is set.
  pub fn state(&self, ctx: &Ctx, id: &str, account: &ApiAccount) -> Element {
    let quiet = PaletteColor::TextSecondary;
    let missing = self
      .missing
      .as_ref()
      .is_some_and(|missing| missing.contains(&account.id()));
    let (colors, key) = if missing {
      ((PaletteColor::Danger, PaletteColor::Danger), "accounts.api.keyMissing")
    } else {
      match account.last_check().map(|check| &check.result) {
        Some(CheckResult::Api { .. }) => ((PaletteColor::Success, quiet), "accounts.api.state.connected"),
        Some(CheckResult::KeyRejected) => (
          (PaletteColor::Danger, PaletteColor::Danger),
          "accounts.api.state.keyRejected",
        ),
        Some(CheckResult::BillingProblem { .. }) => (
          (PaletteColor::Warning, PaletteColor::Warning),
          "accounts.api.state.billing",
        ),
        Some(CheckResult::EndpointUnreachable) => (
          (PaletteColor::Danger, PaletteColor::Danger),
          "accounts.api.state.unreachable",
        ),
        _ => ((PaletteColor::Success, quiet), "accounts.api.keySet"),
      }
    };
    status(id, colors, &t(ctx, key))
      .describe("state", key.rsplit('.').next().unwrap_or_default())
      .into()
  }
}
