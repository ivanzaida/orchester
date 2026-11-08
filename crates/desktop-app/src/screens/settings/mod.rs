//! The app's settings (`App settings · redesign` `iUmo3` in the new `orchester.pen`): the title, the six page tabs
//! *General · Notifications · Accounts · Automation · Browser · Orchester Cloud* (`page`) and the open tab's column of
//! groups under them (`frame`). *General* (`l838Ty`, `general`) holds the theme, the plan limits' warning threshold and
//! this computer. *Notifications* (`fQTFi`, `notifications`) holds what notifies, how, quiet hours and the
//! Orchestrator's daily report (`daily_report`). *Automation* (`V8aAj`) holds *Orchester MCP server* (`server`) and
//! what automation may do (`automation`): what it may always do, and the person's own switches for what it may widen
//! through the MCP server, which no tool can change; what the Orchestrator chat does itself each project sets in its
//! Project settings, which a row there points to (*Let the Orchestrator drive*). *Browser* (`j3Lre`, `NZECi` in
//! `Browser · redesign`, `browser`) holds agents' browser, its profiles' folder and the Browser window's switch. On
//! these four the header says whether every change was saved (`save_state`). *Accounts* (`t8i6m`, `accounts`) lists the
//! person's agent CLI accounts by CLI, an account opening its page and *Add account* its own. *Orchester Cloud*
//! (`vPV73`, `orchester`; its route keeps the id `orchester`) holds the cloud account, invites and the team servers in
//! one column, a server opening its page and *Connect a server* its steps.

mod accounts;
mod automation;
mod browser;
mod daily_report;
mod frame;
mod general;
mod notifications;
pub mod orchester;
mod page;
mod save_state;
mod serial;
mod server;
mod window_rows;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::messages::Message;
use parking_lot::Mutex;

pub(crate) use self::automation::Consent;
use self::save_state::SaveState;
use crate::{
  queries::{self, Again, Session, app},
  routes::{AccountsView, JoinTarget, OrchesterView, UpdatesShow},
  ui::{message::message_text, state::load_error},
};

/// What a route brings into view on Settings › General.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneralShow {
  /// *Plan limits*, as Usage › Plan limits' menu opens it.
  PlanLimits,
  /// *Updates* (`wSMNG`).
  Updates(UpdatesShow),
}

/// The open tab of Settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
  /// Settings › General (`l838Ty`), with *Plan limits* or *Updates* in view when a route says so.
  General(Option<GeneralShow>),
  /// Settings › Notifications (`fQTFi`).
  Notifications,
  /// Settings › Automation (`V8aAj`).
  Automation,
  /// Settings › Browser (`j3Lre`, `NZECi`).
  Browser,
  /// Settings › Accounts (`t8i6m`), an account's page (`xN1Jy`) or *Add account* (`XAm8i`).
  Accounts(AccountsView),
  /// Settings › Orchester Cloud (`vPV73`), a team server's page (`C6qnY`) or *Connect a server* (`MrVql`).
  Orchester(OrchesterView),
  /// Settings › Orchester Cloud's invite landing (`q7dWu` in `ZUuMT`), a sub-page of the Orchester Cloud tab.
  Invite,
}

impl SettingsTab {
  /// Whether the header says if every change was saved: the tabs whose changes save as they are made.
  const fn saves_as_made(self) -> bool {
    matches!(
      self,
      Self::General(_) | Self::Notifications | Self::Automation | Self::Browser
    )
  }
}

#[derive(Clone, PartialEq)]
struct Props {
  tab: SettingsTab,
  /// Automation asks *Rotate the token?* in place (`shpsk`), as its route asks.
  rotation: bool,
  /// The project the invite landing is for, on [`SettingsTab::Invite`].
  invite: Option<JoinTarget>,
}

impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, tab: SettingsTab) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    "settings",
    Props {
      tab,
      rotation: false,
      invite: None,
    },
  )
}

/// Settings › Automation asking *Rotate the token?* under the access token.
pub fn asking_rotation(ctx: &mut Ctx) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    "settings",
    Props {
      tab: SettingsTab::Automation,
      rotation: true,
      invite: None,
    },
  )
}

/// Settings › Orchester Cloud's invite landing for `target`.
pub fn invite(ctx: &mut Ctx, target: JoinTarget) -> Element {
  ctx.mount_keyed::<SettingsScreen>(
    "settings",
    Props {
      tab: SettingsTab::Invite,
      rotation: false,
      invite: Some(target),
    },
  )
}

struct SettingsScreen {
  /// Counts the asks to read the accounts again (*Try again*, the tab's own changes), each of which invalidates their
  /// query; a change of the accounts from anywhere reads them again through `queries`.
  revision: Signal<u64>,
  again: Again,
  /// What the open tab's saves did, for the header.
  saves: SaveState,
  /// The tab the saves are of: another tab starts them again.
  saves_of: Mutex<Option<SettingsTab>>,
}

impl Component for SettingsScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
      again: Again::default(),
      saves: SaveState::new(ctx),
      saves_of: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { tab, rotation, invite } = ctx.props::<Props>().clone();
    self.follow_tab(tab);
    ctx.provide(self.saves.clone());
    ctx.provide(crate::ui::skeleton::FromFirstFrame);
    let session = Session::of(ctx);
    let query = ctx.query(app::agent_accounts(session));
    self.again.follow(self.revision.get(), &query);
    let loaded = queries::state(&query);
    let count = loaded.data.as_ref().map(|accounts| accounts.len());
    let body = match tab {
      SettingsTab::General(show) => general::tab(ctx, show),
      SettingsTab::Notifications => notifications::tab(ctx),
      SettingsTab::Automation => automation::tab(ctx, rotation),
      SettingsTab::Browser => browser::tab(ctx),
      SettingsTab::Orchester(view) => {
        ctx.mount_keyed::<orchester::OrchesterTab>("settings-orchester", orchester::Props { view })
      }
      SettingsTab::Invite => {
        let target = invite.expect("only `invite` opens the landing, and it names the project");
        orchester::landing::tab(ctx, &target)
      }
      SettingsTab::Accounts(view) => match (&loaded.data, &loaded.error) {
        (_, Some(error)) if !loaded.is_pending() => self.failed(ctx, error),
        (Some(accounts), _) => ctx.mount_keyed::<accounts::AccountsTab>(
          "settings-accounts",
          accounts::Props {
            view,
            accounts: Arc::clone(accounts),
            revision: self.revision.clone(),
          },
        ),
        _ => accounts_pane(accounts::reading(ctx)),
      },
    };
    let saved = tab.saves_as_made().then(|| self.saves.summary());
    page::page(ctx, (tab, saved), (count, loaded.is_pending()), body)
  }
}

impl SettingsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.revision.clone();
    accounts_pane(load_error(
      ctx,
      "settings-accounts-load-error",
      (
        &ctx.t("accounts.loadFailed"),
        &ctx.t_args("accounts.loadFailureDetail", [("reason", message_text(ctx, error))]),
      ),
      move || revision.update(|revision| *revision += 1),
    ))
  }

  /// Forgets the saves of the tab that was shown when another one opens.
  fn follow_tab(&self, tab: SettingsTab) {
    let same = |shown: SettingsTab| std::mem::discriminant(&shown) == std::mem::discriminant(&tab);
    let mut saves_of = self.saves_of.lock();
    if !saves_of.is_some_and(same) {
      *saves_of = Some(tab);
      self.saves.clear();
    }
  }
}

/// The Accounts tab's column while the accounts are read, or say why they cannot be.
fn accounts_pane(state: Element) -> Element {
  frame::column(
    "settings-accounts-scroll",
    frame::pane("settings-accounts").child(state),
  )
}
