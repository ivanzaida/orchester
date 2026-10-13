//! *Remove … from Orchester?* in place over *Used by* (`HzJSR`, states in `yEJPZ`): where the account's connections
//! go (another account of its CLI, or *No account*), that finished runs keep its name, and its folder: one Orchester
//! created can go to the Recycle Bin / Trash when ticked, one the person brought stays. While a run is on the account
//! it can't be removed, and Default never can.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, SelectOption, Text},
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
  words::{folder, name, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::{AccountsSpacing, ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::{SelectField, field_error},
    icon::Icon,
    inspector::note,
    message::message_text,
    toggle::{Toggle, ToggleKind},
    unavailable::reason_line,
  },
};

/// The select's width (`Move the connections to` in `HzJSR`).
const MOVE_WIDTH: f32 = 340.0;

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

/// Why Default can't be removed (`yEJPZ`).
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

pub(super) fn confirmation(
  ctx: &mut Ctx,
  parts: &Parts,
  work: &Work,
  account: &AgentAccount,
  uses: Option<&AccountUses>,
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
  let mut question = card("account-remove-question", CardSurface::Decision)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Text::new(&ta(ctx, "accounts.remove.title", [("account", shown.clone())]))
        .variant(ExtraTypography::Strong.role()),
    );
  if let Some(run) = running {
    question = question.child(note(
      "account-remove-running",
      Icon::CirclePlay,
      &ta(
        ctx,
        "accounts.remove.running",
        [("run", run.label.clone()), ("account", shown.clone())],
      ),
    ));
  }
  question = question
    .child(note(
      "account-remove-connections",
      Icon::Plug,
      &connections_line(ctx, &shown, uses),
    ))
    .with_children(running.is_none().then(|| move_select(ctx, work, &others)));
  if running.is_none() {
    question = question.child(note(
      "account-remove-finished",
      Icon::History,
      &ta(ctx, "accounts.remove.finished", [("account", shown.clone())]),
    ));
    question = question.child(folder_lines(ctx, work, account));
  }
  if let Some(error) = work.change_error() {
    question = question.child(field_error("account-remove-error", &message_text(ctx, &error)));
  }
  question = question.child(buttons(ctx, work, account, &shown, running.is_some()));
  let reason = running.map(|run| {
    reason_line(
      "account-remove-reason",
      &ta(ctx, "accounts.remove.runningReason", [("run", run.label.clone())]),
    )
  });
  lurq::components::Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(question)
    .with_children(reason)
    .into()
}

/// How many connections in which projects use the account.
fn connections_line(ctx: &Ctx, shown: &str, uses: Option<&AccountUses>) -> String {
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
  if uses.connections.len() == 1 {
    ta(
      ctx,
      "accounts.remove.connectionsOne",
      [("projects", projects), ("account", shown.to_owned())],
    )
  } else {
    ta(
      ctx,
      "accounts.remove.connections",
      [
        ("count", uses.connections.len().to_string()),
        ("projects", projects),
        ("account", shown.to_owned()),
      ],
    )
  }
}

/// *Move the connections to* (`HzJSR`): another account of the CLI, or *No account*.
fn move_select(ctx: &Ctx, work: &Work, others: &[&AgentAccount]) -> Element {
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
  lurq::components::Column::new()
    .spacing(SpacingSize::Xs)
    .child(Text::new(&t(ctx, "accounts.remove.moveTo")).variant(ExtraTypography::Note.role()))
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
fn folder_lines(ctx: &Ctx, work: &Work, account: &AgentAccount) -> Element {
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
      lurq::components::Column::new()
        .spacing(SpacingSize::Sm)
        .child(note(
          "account-remove-folder",
          Icon::Folder,
          &t(ctx, "accounts.remove.createdFolder"),
        ))
        .child(
          Row::new().padding_left(AccountsSpacing::NoteIndent.role()).child(
            Toggle::new(
              "account-remove-trash",
              ToggleKind::Checkbox,
              work.removal.trash.get(),
              ta(ctx, key, [("folder", path)]),
            )
            .on_change(move |on| trash.set(on)),
          ),
        )
        .into()
    }
    AccountFolder::Brought(_) | AccountFolder::Own => note(
      "account-remove-folder",
      Icon::Folder,
      &ta(ctx, "accounts.remove.broughtFolder", [("folder", path)]),
    )
    .into(),
  }
}

fn buttons(ctx: &mut Ctx, work: &Work, account: &AgentAccount, shown: &str, running: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let confirm = ActionButton::new(
    "account-remove-confirm",
    ButtonKind::Danger,
    ta(ctx, "accounts.remove.confirm", [("account", shown.to_owned())]),
  )
  .icon(Icon::Trash2);
  let confirm = if running {
    confirm.disabled()
  } else if work.change.state().get().is_pending() {
    confirm.loading()
  } else {
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
  };
  let keep = {
    let removal = work.removal.clone();
    ActionButton::new(
      "account-remove-keep",
      ButtonKind::Secondary,
      t(ctx, "accounts.remove.keep"),
    )
    .on_click(move || {
      removal.move_to.set(MoveChoice::Unset);
      removal.trash.set(false);
      navigation.replace(Route::Accounts(AccountsView::of(id, AccountsPanel::UsedBy)));
    })
  };
  Row::new().spacing(SpacingSize::Sm).child(confirm).child(keep).into()
}
