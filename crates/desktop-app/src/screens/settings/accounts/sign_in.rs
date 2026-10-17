//! Step 2 of *Add account* for the shown account (`ohqHW`, states in `GE7a3`): Orchester opens a terminal window
//! running the CLI's own login for the account's folder and waits for it to close, then checks who is signed in; the
//! same command is shown to run in a terminal of one's own. Orchester never sees what is typed there.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::account::{AccountCli, AccountState, AgentAccount, same_sign_in};
use orchester_workspace::{accounts::sign_in::command, display};

use super::{
  Parts, Work,
  add::step,
  details::{frame, glyph, header_row},
  state::{command_row, copy_folder},
  words::{Shown, cli_key, login_command, method_key, name, plan, status_command, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::{AccountsSpacing, AccountsText, ExtraRadius, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::{Icon, icon},
    inspector::note,
    message::message_text,
  },
};

pub(super) fn panel(ctx: &mut Ctx, parts: &Parts, work: &Work) -> Element {
  let Some(account) = parts.view.account.and_then(|id| parts.account(id)) else {
    return frame("account-sign-in").into();
  };
  let shown = name(ctx, account);
  let program = account.cli().program().to_owned();
  let own = command(account);
  let error = work
    .sign_in
    .state()
    .get()
    .error
    .map(|error| field_error("account-sign-in-error", &message_text(ctx, &error)));
  // Signed in, or without the CLI, there is nothing left to run in a terminal: no command, no footer (`GE7a3`).
  let done = matches!(
    parts.shown(account),
    Shown::State(AccountState::SignedIn | AccountState::CliMissing)
  );
  let own_terminal = (!done).then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Text::new(&ta(ctx, "accounts.signIn.ownTerminal", [("shell", own.shell)]))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::TextSecondary),
      )
      .child(command_row(ctx, "account-sign-in-own", &own.text))
      .child(
        Text::new(&ta(ctx, "accounts.signIn.ownNote", [("program", program.clone())]))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
  });
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(AccountsSpacing::Panel.role()))
    .spacing(AccountsSpacing::Parts.role())
    .child(steps(ctx, account, &shown))
    .child(status(ctx, parts, work, account))
    .with_children(error)
    .with_children(own_terminal)
    .child(note(
      "account-sign-in-privacy",
      Icon::ShieldCheck,
      &ta(
        ctx,
        "accounts.signIn.privacy",
        [
          ("program", program),
          ("account", shown.clone()),
          ("statusCommand", status_command(account.cli())),
        ],
      ),
    ));
  frame("account-sign-in")
    .child(
      header_row("account-sign-in-header").child(glyph(Icon::Plus)).child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Xs)
          .child(Text::new(&t(ctx, "accounts.add.title")).variant(AccountsText::Title.role()))
          .child(
            Text::new(&t(ctx, "accounts.add.sub"))
              .variant(ExtraTypography::Prose.role())
              .width(Dimension::Pct(100.0)),
          ),
      ),
    )
    .child(ScrollVertical::new(body).id("account-sign-in-scroll").flex(1.0))
    .with_children((!done).then(|| footer(ctx, parts, work, account, &shown)))
    .into()
}

/// Step 1 done with what it did, step 2 current.
fn steps(ctx: &Ctx, account: &AgentAccount, shown: &str) -> Element {
  let done_key = match account.folder() {
    orchester_core::account::AccountFolder::Created(_) => "accounts.add.step1Created",
    _ => "accounts.add.step1Brought",
  };
  Row::new()
    .id("account-sign-in-steps")
    .width(Dimension::Pct(100.0))
    .spacing(AccountsSpacing::Cell.role())
    .child(step(
      "account-add-step-1",
      Icon::CircleCheck,
      &t(ctx, "accounts.add.step1"),
      false,
      Some(&ta(ctx, done_key, [("name", shown.to_owned())])),
    ))
    .child(step(
      "account-add-step-2",
      Icon::CircleDot,
      &t(ctx, "accounts.add.step2"),
      true,
      None,
    ))
    .into()
}

/// Where the sign-in stands: the terminal open, signed in, still signed out, the CLI missing, or not opened yet.
fn status(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let shown = name(ctx, account);
  let cli = account.cli();
  let program = cli.program().to_owned();
  let id = account.id();
  let check_again = {
    let work = work.clone();
    let button = ActionButton::new(
      "account-sign-in-check",
      ButtonKind::Secondary,
      t(ctx, "accounts.actions.checkAgain"),
    )
    .icon(Icon::RefreshCw);
    if parts.checking.contains(&id) {
      button.loading()
    } else {
      button.on_click(move || work.check_again(vec![id]))
    }
  };
  let state = parts.shown(account);
  if state == Shown::SigningIn {
    let waiting = waiting(ctx, work, id, check_again);
    let text = ta(
      ctx,
      "accounts.signIn.opened",
      [("command", login_command(cli)), ("account", shown)],
    );
    return terminal_card(ctx, &program, &text).child(waiting).into();
  }
  match state {
    Shown::State(AccountState::SignedIn) => signed_in(ctx, parts, account),
    Shown::State(AccountState::CliMissing) => outcome(
      "account-sign-in-cli-missing",
      (Icon::CircleX, PaletteColor::Danger),
      &ta(ctx, "accounts.signIn.cliMissing", [("program", program)]),
      &ta(ctx, "accounts.signIn.cliMissingBody", [("cli", t(ctx, cli_key(cli)))]),
    )
    .child(Row::new().child(check_again))
    .into(),
    _ if work.sign_in.state().get().is_fulfilled() => outcome(
      "account-sign-in-still-out",
      (Icon::CircleAlert, PaletteColor::Warning),
      &ta(
        ctx,
        "accounts.signIn.stillOut",
        [("program", program), ("account", shown)],
      ),
      &t(ctx, "accounts.signIn.stillOutBody"),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .child(open_button(ctx, work, id, true))
        .child(check_again),
    )
    .into(),
    _ => {
      let text = ta(
        ctx,
        "accounts.signIn.notOpened",
        [("command", login_command(cli)), ("account", shown)],
      );
      terminal_card(ctx, &program, &text)
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .child(open_button(ctx, work, id, false))
            .child(check_again),
        )
        .into()
    }
  }
}

/// *Waiting for the terminal to close · opened …* with *Check again* at its end.
fn waiting(ctx: &Ctx, work: &Work, id: orchester_core::id::AgentAccountId, check_again: ActionButton) -> Row {
  let opened = work.opened_at(id).map(display::clock).unwrap_or_default();
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::LoaderCircle, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ta(ctx, "accounts.signIn.waiting", [("time", opened)]))
        .id("account-sign-in-waiting")
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0),
    )
    .child(check_again)
}

/// *Sign in with …'s own login*: its title and what Orchester does.
fn terminal_card(ctx: &Ctx, program: &str, text: &str) -> lurq::components::Column {
  card("account-sign-in-terminal", CardSurface::Raised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::SquareTerminal, IconSize::Base, PaletteColor::TextPrimary))
        .child(
          Text::new(&ta(ctx, "accounts.signIn.title", [("program", program.to_owned())]))
            .variant(ExtraTypography::Strong.role()),
        ),
    )
    .child(
      Text::new(text)
        .id("account-sign-in-text")
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// A result of the sign-in: its mark, title and what it means (`GE7a3`).
fn outcome(id: &str, (glyph, color): (Icon, PaletteColor), title: &str, body: &str) -> lurq::components::Column {
  card(id, CardSurface::Raised)
    .rounded(ExtraRadius::Tile.role())
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, color))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(ExtraTypography::Strong.role()),
        ),
    )
    .child(
      Text::new(body)
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// Signed in as the same person as `other`: said plainly, with *Keep both* and *Remove* the new one (`GE7a3`).
fn same_person(ctx: &mut Ctx, account: &AgentAccount, other: &AgentAccount, email: String) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let shown = name(ctx, account);
  let to = navigation.clone();
  let remove = navigation;
  outcome(
    "account-sign-in-same",
    (Icon::Users, PaletteColor::TextSecondary),
    &ta(
      ctx,
      "accounts.signIn.sameTitle",
      [("email", email), ("account", name(ctx, other))],
    ),
    &t(ctx, "accounts.signIn.sameBody"),
  )
  .child(
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(
          "account-sign-in-keep-both",
          ButtonKind::Secondary,
          t(ctx, "accounts.signIn.keepBoth"),
        )
        .icon(Icon::Check)
        .on_click(move || to.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)))),
      )
      .child(
        ActionButton::new(
          "account-sign-in-remove",
          ButtonKind::Ghost,
          ta(ctx, "accounts.signIn.removeIt", [("account", shown)]),
        )
        .icon(Icon::Trash2)
        .on_click(move || remove.go(Route::Accounts(AccountsView::of(id, AccountsPanel::Remove)))),
      ),
  )
  .into()
}

/// Signed in: who, and *Done*; the same person as another account says so and offers to remove the new one.
fn signed_in(ctx: &mut Ctx, parts: &Parts, account: &AgentAccount) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let shown = name(ctx, account);
  let email = account.email().map(str::to_owned);
  let checked = account
    .last_check()
    .map(|check| display::clock(check.at))
    .unwrap_or_default();
  if let (Some(other), Some(email)) = (same_sign_in(account, &parts.accounts), email.clone()) {
    return same_person(ctx, account, other, email);
  }
  let title = email.map_or_else(
    || t(ctx, "accounts.signIn.signedInNoEmail"),
    |email| ta(ctx, "accounts.signIn.signedIn", [("email", email)]),
  );
  let facts = identity_facts(ctx, account);
  let body = if facts.is_empty() {
    ta(
      ctx,
      "accounts.signIn.signedInBody",
      [("time", checked), ("account", shown.clone())],
    )
  } else {
    ta(
      ctx,
      "accounts.signIn.signedInFactsBody",
      [("facts", facts), ("time", checked), ("account", shown.clone())],
    )
  };
  // A Claude Code folder is what a person points their own claude at (`GE7a3`).
  let copy =
    (account.cli() == AccountCli::ClaudeCode).then(|| copy_folder(ctx, account, "account-sign-in-copy-folder"));
  outcome(
    "account-sign-in-done",
    (Icon::CircleCheck, PaletteColor::Success),
    &title,
    &body,
  )
  .child(
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(
          "account-sign-in-finish",
          ButtonKind::Primary,
          t(ctx, "accounts.signIn.done"),
        )
        .icon(Icon::ArrowRight)
        .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)))),
      )
      .with_children(copy),
  )
  .child(
    Text::new(&ta(
      ctx,
      "accounts.signIn.next",
      [("account", shown), ("cli", t(ctx, cli_key(account.cli())))],
    ))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0)),
  )
  .into()
}

/// The plan and how the account signed in, as its CLI reported them (`Claude Pro plan · signed in with claude.ai`);
/// empty when it reported neither.
fn identity_facts(ctx: &Ctx, account: &AgentAccount) -> String {
  let identity = account.last_check().and_then(|check| check.known_identity());
  let plan = identity.and_then(|identity| identity.plan.as_deref()).map(plan);
  let method = identity
    .and_then(|identity| identity.method)
    .map(|method| t(ctx, method_key(method)));
  match (plan, method) {
    (Some(plan), Some(method)) => ta(
      ctx,
      "accounts.signIn.planAndMethod",
      [("plan", plan), ("method", method)],
    ),
    (Some(plan), None) => ta(ctx, "accounts.signIn.planFact", [("plan", plan)]),
    (None, Some(method)) => ta(ctx, "accounts.signIn.methodFact", [("method", method)]),
    (None, None) => String::new(),
  }
}

fn open_button(ctx: &Ctx, work: &Work, id: orchester_core::id::AgentAccountId, again: bool) -> ActionButton {
  let key = if again {
    "accounts.signIn.openAgain"
  } else {
    "accounts.signIn.open"
  };
  let button = ActionButton::new(
    if again {
      "account-sign-in-open-again"
    } else {
      "account-sign-in-open"
    },
    ButtonKind::Secondary,
    t(ctx, key),
  )
  .icon(Icon::SquareTerminal);
  if work.another_signing_in(id) {
    return button.unavailable(t(ctx, "accounts.signIn.another"));
  }
  let work = work.clone();
  button.on_click(move || work.open_sign_in(id))
}

/// What is saved, *Finish later* and *Open the terminal again*.
fn footer(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount, shown: &str) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let finish = ActionButton::new(
    "account-sign-in-later",
    ButtonKind::Secondary,
    t(ctx, "accounts.signIn.finishLater"),
  )
  .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account))));
  let again = if parts.signing_in == Some(id) {
    open_button(ctx, work, id, true).loading()
  } else {
    open_button(ctx, work, id, true)
  };
  Row::new()
    .id("account-sign-in-footer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Panel.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&ta(ctx, "accounts.signIn.saved", [("account", shown.to_owned())]))
        .variant(ExtraTypography::Note.role()),
    )
    .child(Spacer::new().flex(1.0))
    .child(finish)
    .child(again)
    .into()
}
