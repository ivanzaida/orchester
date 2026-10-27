//! The list of accounts (`03 · Accounts` `t8i6m`): a group per CLI, its heading with how many accounts it has (and
//! *Add account* at the first one's end), a row per account with the CLI maker's mark in its tile, its name, who is
//! signed in under it, its state and a chevron: a press opens its page. The accounts the person added come first,
//! Default (the CLI's own folder) last. The note under the groups says whose accounts they are.

use lurq::{app::ctx::Ctx, components::Column, node::Element};
use orchester_core::account::{AccountCli, AccountState, AgentAccount};

use super::{
  Parts,
  words::{Shown, cli_key, identity, look, name, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::IconSize,
  ui::{
    agent_mark::{AgentMark, mark_icon},
    button::{ActionButton, ButtonKind},
    icon::Icon,
    setting_rows::{SettingGroup, SettingRow, group_note, opens_mark, row_tile, status},
  },
};

/// The list page's column.
pub(super) fn page(ctx: &mut Ctx, parts: &Parts) -> Column {
  let navigation = Navigation::of(ctx);
  let mut pane = super::frame::pane("settings-accounts");
  for (index, cli) in AccountCli::ALL.into_iter().enumerate() {
    // Default last: the accounts the person added come first.
    let mut accounts: Vec<&AgentAccount> = parts.accounts.iter().filter(|account| account.cli() == cli).collect();
    accounts.sort_by_key(|account| account.is_default());
    let count = if accounts.len() == 1 {
      t(ctx, "accounts.group.one")
    } else {
      ta(ctx, "accounts.group.other", [("count", accounts.len().to_string())])
    };
    let rows: Vec<Element> = accounts
      .into_iter()
      .map(|account| row(ctx, parts, account, &navigation))
      .collect();
    let mut group = SettingGroup::new(format!("accounts-group-{}", cli.as_str()), t(ctx, cli_key(cli)))
      .meta(count)
      .rows(rows);
    if index == 0 {
      group = group.end(
        ActionButton::new("settings-add-account", ButtonKind::Ghost, t(ctx, "accounts.addAccount"))
          .icon(Icon::Plus)
          .small()
          .opens(navigation.to(Route::Accounts(AccountsView {
            account: None,
            panel: AccountsPanel::Add,
          }))),
      );
    }
    pane = pane.child(group);
  }
  pane.child(group_note("accounts-note", &t(ctx, "accounts.note")))
}

/// An account's state as a dot and its word, in the state's colors.
pub(super) fn state_line(ctx: &Ctx, id: &str, shown: Shown) -> Element {
  let look = look(shown);
  status(id, (look.icon_color, look.color), &t(ctx, look.key))
    .describe("state", look.key.rsplit('.').next().unwrap_or_default())
    .into()
}

/// The CLI maker's mark in a row's tile.
pub(super) fn mark_tile(ctx: &Ctx, cli: AccountCli) -> Element {
  row_tile(mark_icon(ctx, AgentMark::of_cli(cli), IconSize::Md.size())).into()
}

fn row(ctx: &mut Ctx, parts: &Parts, account: &AgentAccount, navigation: &Navigation) -> Element {
  let id = format!("account-row-{}", account.id());
  let shown = parts.shown(account);
  let line = identity(ctx, account, &parts.accounts, shown);
  // Default says it is the CLI's own folder before who is signed in (`t8LAtj`); signed out, its line says so already.
  let signed_out = matches!(
    shown,
    Shown::State(AccountState::NotSignedIn | AccountState::NotChecked)
  );
  let line = if account.is_default() && !signed_out {
    ta(ctx, "accounts.identity.ownFolder", [("identity", line)])
  } else {
    line
  };
  let state = lurq::components::Row::new()
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(lurq::layout::Alignment::Center)
    .child(state_line(ctx, &format!("{id}-state"), shown))
    .child(opens_mark());
  SettingRow::new(id, name(ctx, account))
    .lead(mark_tile(ctx, account.cli()))
    .hint(line)
    .control(state)
    .opens(navigation.to(Route::Accounts(AccountsView::of(account.id(), AccountsPanel::Account))))
    .render(ctx)
}
