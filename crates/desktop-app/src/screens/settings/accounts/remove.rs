//! *Remove … from Orchester?* asked in place under the account page's head (`Remove` `CTa4g` in `poD7g`): which
//! connections use the account and where they go (another account of its CLI, or *No account*), that finished runs
//! keep its name, and its folder: one Orchester created can go to the Recycle Bin / Trash when ticked, one the person
//! brought stays. While a run is on the account, or a project that may hold one can't be read, it can't be removed
//! and a notice says why instead (`Notice · info` `w415U`); Default never can.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, SelectOption, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountFolder, AgentAccount, MoveTo},
  connection::invocation::Platform,
  id::AgentAccountId,
};
use orchester_workspace::accounts::AccountUses;

use super::{
  Parts, Work,
  state::paragraph,
  words::{folder, name, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{SelectField, field_error},
    icon::Icon,
    message::message_text,
    notice::NoticeTone,
    setting_notice::{SettingNotice, SettingPanel},
    toggle::{Toggle, ToggleKind},
  },
};

/// The select's width (`Box` `P6iZMy`).
const MOVE_WIDTH: f32 = 320.0;

/// Where the connections of the removed account go.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MoveChoice {
  /// Not chosen yet: the first other account of the CLI.
  #[default]
  Unset,
  To(AgentAccountId),
  NoAccount,
}

impl DevtoolsInspectable for MoveChoice {}

/// The person's choices for the removal.
#[derive(Clone)]
pub(super) struct Choice {
  pub move_to: Signal<MoveChoice>,
  pub trash: Signal<bool>,
}

impl Choice {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      move_to: ctx.signal(MoveChoice::Unset),
      trash: ctx.signal(false),
    }
  }
}

/// Why Default can't be removed.
pub(super) fn default_reason(ctx: &Ctx, account: &AgentAccount) -> String {
  let program = account.cli().program();
  let folder = folder(account)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  ta(
    ctx,
    "accounts.remove.defaultReason",
    [("program", program.to_owned()), ("folder", folder)],
  )
}

/// The question, or why the account can't be removed; while what uses it is still being read, *Remove …* waits, so
/// the question is never answered on connections it has not read.
pub(super) fn confirmation(
  ctx: &mut Ctx,
  (parts, work): (&Parts, &Work),
  account: &AgentAccount,
  (uses, reading): (Option<&AccountUses>, bool),
) -> Element {
  let shown = name(ctx, account);
  let others: Vec<&AgentAccount> = parts
    .accounts
    .iter()
    .filter(|other| other.cli() == account.cli() && other.id() != account.id())
    .collect();
  if work.removal.move_to.get() == MoveChoice::Unset {
    work.removal.move_to.set(
      others
        .first()
        .map_or(MoveChoice::NoAccount, |other| MoveChoice::To(other.id())),
    );
  }
  let running = uses.and_then(|uses| uses.running.first());
  let unreadable: Vec<String> = uses
    .map(|uses| uses.unreadable.iter().map(ToString::to_string).collect())
    .unwrap_or_default();
  let blocked = match running {
    Some(run) => Some(ta(ctx, "accounts.remove.runningReason", [("run", run.label.clone())])),
    // A run in a project that can't be read may be on the account.
    None if !unreadable.is_empty() => Some(ta(
      ctx,
      "accounts.remove.unreadableReason",
      [("projects", unreadable.join(", "))],
    )),
    None => None,
  };
  let connections = connections_line(ctx, &shown, uses, blocked.is_some());
  if let Some(reason) = blocked {
    let notice = SettingNotice::new("account-remove-blocked", NoticeTone::Info);
    let notice = match running {
      Some(run) => notice.body(ta(
        ctx,
        "accounts.remove.running",
        [("run", run.label.clone()), ("account", shown)],
      )),
      None => notice.title(ta(ctx, "accounts.remove.title", [("account", shown)])),
    };
    return notice
      .part(paragraph("account-remove-connections", &connections))
      .action(keep(ctx, work, account.id()))
      .note(reason)
      .into();
  }
  let mut panel = SettingPanel::new(
    "account-remove-question",
    ta(ctx, "accounts.remove.title", [("account", shown.clone())]),
  )
  .danger()
  .text("account-remove-connections", &connections);
  if uses.is_some_and(|uses| !uses.connections.is_empty()) {
    panel = panel.part(move_select(ctx, work, &others));
  }
  panel = panel.note(
    "account-remove-finished",
    &ta(ctx, "accounts.remove.finished", [("account", shown.clone())]),
  );
  panel = folder_lines(ctx, work, account, panel);
  if let Some(error) = work.change_error() {
    panel = panel.part(field_error("account-remove-error", &message_text(ctx, &error)));
  }
  let confirm = confirm(ctx, work, account, &shown);
  let confirm = if reading { confirm.disabled() } else { confirm };
  let keep = keep(ctx, work, account.id());
  panel.actions([confirm.into(), keep.into()]).into()
}

/// Which connections use the account and where they go: to the account the person chooses, or, while it can't be
/// removed, where the person chooses once it can.
fn connections_line(ctx: &Ctx, shown: &str, uses: Option<&AccountUses>, blocked: bool) -> String {
  let Some(uses) = uses.filter(|uses| !uses.connections.is_empty()) else {
    return ta(ctx, "accounts.remove.none", [("account", shown.to_owned())]);
  };
  let mut projects: Vec<String> = uses
    .connections
    .iter()
    .map(|used| used.project_name.to_string())
    .collect();
  projects.dedup();
  let projects = projects.join(", ");
  let (one, many) = if blocked {
    (
      "accounts.remove.connectionsOneBlocked",
      "accounts.remove.connectionsBlocked",
    )
  } else {
    ("accounts.remove.connectionsOne", "accounts.remove.connections")
  };
  if uses.connections.len() == 1 {
    ta(ctx, one, [("projects", projects), ("account", shown.to_owned())])
  } else {
    ta(
      ctx,
      many,
      [
        ("count", uses.connections.len().to_string()),
        ("projects", projects),
        ("account", shown.to_owned()),
      ],
    )
  }
}

/// *Move the connections to* (`Move the connections to` `aFuFq`): another account of the CLI, or *No account*.
fn move_select(ctx: &mut Ctx, work: &Work, others: &[&AgentAccount]) -> Element {
  let mut options: Vec<SelectOption<MoveChoice>> = others
    .iter()
    .map(|other| {
      let label = match other.email() {
        Some(email) => format!("{} · {email}", name(ctx, other)),
        None => name(ctx, other),
      };
      SelectOption::new(MoveChoice::To(other.id()), label)
    })
    .collect();
  options.push(SelectOption::new(
    MoveChoice::NoAccount,
    t(ctx, "accounts.remove.noAccount"),
  ));
  Column::new()
    .spacing(SpacingSize::Xs)
    .child(Text::new(&t(ctx, "accounts.remove.moveTo")).variant(TypographyStyle::Caption))
    .child(
      SelectField::new(
        "account-remove-move-to",
        t(ctx, "accounts.remove.moveTo"),
        work.removal.move_to.clone(),
        options,
      )
      .width(Dimension::Px(MOVE_WIDTH))
      .render(ctx),
    )
    .into()
}

/// What happens to the folder: one Orchester created may go to the Recycle Bin / Trash, one brought stays.
fn folder_lines(ctx: &Ctx, work: &Work, account: &AgentAccount, panel: SettingPanel) -> SettingPanel {
  let path = folder(account)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  match account.folder() {
    AccountFolder::Created(_) => {
      let key = match Platform::current() {
        Platform::Windows => "accounts.remove.trashWindows",
        Platform::MacOs => "accounts.remove.trashMac",
      };
      let trash = work.removal.trash.clone();
      panel
        .part(
          Toggle::new(
            "account-remove-trash",
            ToggleKind::Checkbox,
            work.removal.trash.get(),
            ta(ctx, key, [("folder", path)]),
          )
          .wrap_label()
          .on_change(move |on| trash.set(on)),
        )
        .note("account-remove-folder", &t(ctx, "accounts.remove.createdFolder"))
    }
    AccountFolder::Brought(_) | AccountFolder::Own => panel.note(
      "account-remove-folder",
      &ta(ctx, "accounts.remove.broughtFolder", [("folder", path)]),
    ),
  }
}

/// *Remove …*, busy while it removes.
fn confirm(ctx: &Ctx, work: &Work, account: &AgentAccount, shown: &str) -> ActionButton {
  let id = account.id();
  let confirm = ActionButton::new(
    "account-remove-confirm",
    ButtonKind::Danger,
    ta(ctx, "accounts.remove.confirm", [("account", shown.to_owned())]),
  )
  .icon(Icon::Trash2);
  if work.change.state().get().is_pending() {
    return confirm.loading();
  }
  let (change, removal) = (work.change.clone(), work.removal.clone());
  confirm.on_click(move || {
    let move_to = match removal.move_to.get() {
      MoveChoice::To(target) => MoveTo::Account(target),
      MoveChoice::Unset | MoveChoice::NoAccount => MoveTo::NoAccount,
    };
    change.run(Change::Remove {
      account: id,
      move_to,
      trash: removal.trash.get(),
    });
  })
}

/// *Keep it*: the question closes and the choices are forgotten.
fn keep(ctx: &mut Ctx, work: &Work, id: AgentAccountId) -> ActionButton {
  let navigation = Navigation::of(ctx);
  let removal = work.removal.clone();
  ActionButton::new("account-remove-keep", ButtonKind::Ghost, t(ctx, "accounts.remove.keep")).on_click(move || {
    removal.move_to.set(MoveChoice::Unset);
    removal.trash.set(false);
    navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)));
  })
}
