//! The page of Settings (`Header` `t7cJ0W` and `Tabs` `xEFQ0` in `01 · General` `l838Ty` of the new
//! `orchester.pen`): the title with what Settings is on its line and, at its end, whether the open tab's changes were
//! saved (General, Notifications, Automation and Browser, whose changes save as they are made), then the six page tabs
//! *General · Notifications · Accounts · Automation · Browser · Orchester Cloud* (`EPO9U` in `09 · Settings · Browser`
//! `j3Lre`), each a place of its own, and the open tab under them. A sub-page of a tab keeps its tab current; a press
//! on the current tab goes back to the tab itself.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use super::{
  SettingsTab, frame,
  save_state::{self, Summary},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, OrchesterPanel, OrchesterView, Route, Screen},
  ui::tabs::{PageTab, page_tabs},
};

/// Settings on `tab`, showing `body`, with what its saves did (`saved`) on a tab whose changes save as they are made;
/// `accounts` is how many accounts there are, once read, and whether they are being read for the first time.
pub(super) fn page(
  ctx: &mut Ctx,
  (tab, saved): (SettingsTab, Option<Summary>),
  accounts: (Option<usize>, bool),
  body: Element,
) -> Element {
  let navigation = Navigation::of(ctx);
  let end = saved.map(|summary| save_state::label(ctx, summary));
  let tabs = tabs(ctx, tab, accounts, &navigation);
  frame::page(
    &format!("screen-{}", Screen::Settings.name()),
    (&ctx.t(Screen::Settings.title_key()), &ctx.t("settings.sub")),
    end,
    tabs,
    body,
  )
}

fn tabs(
  ctx: &mut Ctx,
  tab: SettingsTab,
  (accounts, reading): (Option<usize>, bool),
  navigation: &Navigation,
) -> Element {
  // Each tab is a place of its own. The current one opens nothing, unless a sub-page of it shows: then it goes back.
  let tab_to = |id: &str, label: Arc<str>, (current, at_tab): (bool, bool), route: Route| {
    if current && at_tab {
      PageTab::current(id.to_owned(), label)
    } else {
      PageTab::to(id.to_owned(), label, current, navigation.to(route))
    }
  };
  // How many accounts there are, once read, is a muted number after the tab's label (`mIlfH`).
  let accounts_tab = tab_to(
    "settings-tab-accounts",
    ctx.t("settings.tabs.accounts"),
    match tab {
      SettingsTab::Accounts(view) => (true, view == AccountsView::default()),
      _ => (false, true),
    },
    Route::Accounts(AccountsView::default()),
  );
  let accounts_tab = match accounts {
    Some(count) => accounts_tab.count(count, false),
    None if reading => accounts_tab.count_reading(),
    None => accounts_tab,
  };
  let tabs = [
    tab_to(
      "settings-tab-general",
      ctx.t("settings.tabs.general"),
      (tab == SettingsTab::General, true),
      Route::General,
    ),
    tab_to(
      "settings-tab-notifications",
      ctx.t("settings.tabs.notifications"),
      (tab == SettingsTab::Notifications, true),
      Route::Notifications,
    ),
    accounts_tab,
    tab_to(
      "settings-tab-automation",
      ctx.t("settings.tabs.automation"),
      (tab == SettingsTab::Automation, true),
      Route::Settings,
    ),
    tab_to(
      "settings-tab-browser",
      ctx.t("settings.tabs.browser"),
      (tab == SettingsTab::Browser, true),
      Route::Browser,
    ),
    tab_to(
      "settings-tab-orchester",
      ctx.t("settings.tabs.orchester"),
      match tab {
        SettingsTab::Orchester(view) => (true, view.server.is_none() && view.panel != OrchesterPanel::Connect),
        SettingsTab::Invite => (true, false),
        _ => (false, true),
      },
      Route::Orchester(OrchesterView::default()),
    ),
  ];
  page_tabs(ctx, "settings-tabs", tabs).into()
}
