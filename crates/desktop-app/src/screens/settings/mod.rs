//! The app's settings: a header with its tabs (`General · Automation · Accounts · Orchester`, `t098v`, `lye6f` and
//! `Cu20b`) over the open tab. *Automation* (`lye6f`) holds *Orchester MCP server* (`server`) and *What automation may
//! do*: the person's own switches for what automation may widen through the MCP server and whether the Orchestrator
//! chat publishes its drafts itself, which no tool can change. *Accounts* (`accounts`, layer `hgOl2`) holds the
//! person's agent CLI accounts. *Orchester* (`orchester`) holds the Orchester Cloud account and the team servers, and
//! *Connect a server* replaces the whole page. *General* is not built in this app yet and says so.

mod accounts;
mod automation;
mod notifications;
pub mod orchester;
mod page;
mod server;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::account::AgentAccount;
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use crate::{
  routes::{AccountsView, OrchesterPanel, OrchesterView, Screen},
  services::Services,
  ui::{
    message::message_text,
    settings_page::form_scroll,
    state::{load_error, loading},
  },
};

/// The accounts as read, shared with the tab.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Listed(Arc<Vec<AgentAccount>>);

impl DevtoolsInspectable for Listed {}

/// The open tab of Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
  Automation,
  Accounts(AccountsView),
  /// Settings › Orchester (`Cu20b`), or *Connect a server* (`zxe2P`).
  Orchester(OrchesterView),
}

#[derive(Clone, Copy, PartialEq)]
struct Props {
  tab: SettingsTab,
}

impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, tab: SettingsTab) -> Element {
  ctx.mount_keyed::<SettingsScreen>("settings", Props { tab })
}

struct SettingsScreen {
  /// Counts the changes to the accounts, each of which reads them again.
  revision: Signal<u64>,
}

impl Component for SettingsScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tab = ctx.props::<Props>().tab;
    if let SettingsTab::Orchester(view) = tab
      && view.panel == OrchesterPanel::Connect
    {
      // *Connect a server* is a page of its own, without Settings' header and tabs (`zxe2P`).
      return Column::new()
        .id(format!("screen-{}", Screen::Settings.name()))
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0))
        .child(ctx.mount_keyed::<orchester::connect::ConnectPage>(
          "settings-orchester-connect",
          orchester::connect::Props { server: view.server },
        ))
        .into();
    }
    let workspace = Services::of(ctx).workspace;
    let loaded = ctx
      .future(self.revision.get(), move |_| {
        let workspace = workspace.clone();
        async move {
          background::run(move || workspace.agent_accounts())
            .await?
            .map(|accounts| Listed(Arc::new(accounts)))
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let count = loaded.data.as_ref().map(|accounts| accounts.0.len());
    let body = match tab {
      SettingsTab::Automation => automation(ctx),
      SettingsTab::Orchester(view) => {
        ctx.mount_keyed::<orchester::OrchesterTab>("settings-orchester", orchester::Props { view })
      }
      SettingsTab::Accounts(view) => match (&loaded.data, &loaded.error) {
        (_, Some(error)) if !loaded.is_pending() => self.failed(ctx, error),
        (Some(accounts), _) => ctx.mount_keyed::<accounts::AccountsTab>(
          "settings-accounts",
          accounts::Props {
            view,
            accounts: Arc::clone(&accounts.0),
            revision: self.revision.clone(),
          },
        ),
        _ => loading("settings-accounts-loading", &ctx.t("accounts.reading")).into(),
      },
    };
    page::page(ctx, tab, count, body)
  }
}

impl SettingsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.revision.clone();
    load_error(
      ctx,
      "settings-accounts-load-error",
      (
        &ctx.t("accounts.loadFailed"),
        &ctx.t_args("accounts.loadFailureDetail", [("reason", message_text(ctx, error))]),
      ),
      move || revision.update(|revision| *revision += 1),
    )
  }
}

/// *Orchester MCP server*, *What automation may do* and *Notifications*, scrolling under the header: the switches sit
/// below the server's section, out of a window's height.
fn automation(ctx: &mut Ctx) -> Element {
  let form = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .child(ctx.mount_keyed::<server::ServerSection>("mcp-server", ()))
    .child(automation::section(ctx))
    .child(ctx.mount_keyed::<notifications::NotificationsSection>("notifications", ()));
  form_scroll("settings-automation-scroll", form).into()
}
