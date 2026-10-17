//! *Add account* in place of the details (`IOyJl`): step 1 of 2 chooses the CLI, the name and the folder, a new one
//! Orchester creates in its application data or one the person already uses, which must stay outside every project
//! and repository (`GE7a3`). *Create folder and sign in* saves the account and goes on to step 2 (`sign_in`); a
//! brought folder is checked at once instead.

use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  account::{AccountCli, FolderProblem},
  connection::invocation::Platform,
  name::Name,
};
use orchester_workspace::{
  accounts::{AccountsError, NewAccount, NewFolder},
  background,
  messages::Message,
};

use super::{
  Work,
  details::{frame, glyph, header_row},
  list::Adding,
  words::{cli_key, own_folder_text, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  services::Services,
  theme::{AccountsSpacing, AccountsText, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    inspector::note,
    message::message_text,
    option_card::OptionCard,
  },
};

/// The chosen CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CliChoice(pub AccountCli);
impl DevtoolsInspectable for CliChoice {}

/// Where the new account's folder comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FolderChoice {
  New,
  Existing,
}
impl DevtoolsInspectable for FolderChoice {}

/// The folder the system's picker gave, if it gave one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Picked(Option<PathBuf>);
impl DevtoolsInspectable for Picked {}

type PickAction = FutureAction<(), Picked, Message>;

/// What the person has chosen so far.
#[derive(Clone)]
pub(super) struct Draft {
  pub cli: Signal<CliChoice>,
  pub name: Signal<String>,
  pub folder: Signal<FolderChoice>,
  /// The brought folder's path, as typed or chosen.
  pub path: Signal<String>,
  pick: PickAction,
}

impl Draft {
  pub fn new(ctx: &mut Ctx) -> Self {
    let first = AccountCli::ALL
      .into_iter()
      .find(|cli| cli.supports_accounts_on(Platform::current()))
      .unwrap_or(AccountCli::Codex);
    let draft = Self {
      cli: ctx.signal(CliChoice(first)),
      name: ctx.signal(String::new()),
      folder: ctx.signal(FolderChoice::New),
      path: ctx.signal(String::new()),
      pick: ctx
        .future_action(|()| async move { background::run(move || Picked(rfd::FileDialog::new().pick_folder())).await }),
    };
    let path = draft.path.clone();
    ctx.watch(&draft.pick.state(), move |state| {
      if let Some(Picked(Some(chosen))) = state.data.clone().filter(|_| state.is_fulfilled()) {
        path.set(chosen.display().to_string());
      }
    });
    draft
  }

  /// The row the list shows for the account being added.
  pub fn adding(&self) -> Adding {
    Adding {
      cli: self.cli.get().0,
      name: self.name.get(),
    }
  }
}

pub(super) fn panel(ctx: &mut Ctx, work: &Work) -> Element {
  let draft = &work.draft;
  let cli = draft.cli.get().0;
  let problem = folder_problem(ctx, draft, cli);
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(AccountsSpacing::Panel.role()))
    .spacing(AccountsSpacing::Parts.role())
    .child(steps(ctx))
    .child(cli_choice(ctx, draft))
    .child(name_field(ctx, draft, cli))
    .child(folder_choice(ctx, draft, cli, problem.as_ref()))
    .child(note(
      "account-add-note",
      Icon::ShieldCheck,
      &ta(ctx, "accounts.add.folderNote", [("folder", own_folder_text(cli))]),
    ));
  frame("account-add")
    .child(
      header_row("account-add-header").child(glyph(Icon::Plus)).child(
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
    .child(ScrollVertical::new(body).id("account-add-scroll").flex(1.0))
    .child(footer(ctx, work, problem.is_some()))
    .into()
}

/// *1. Name and folder*, current, beside *2. Sign in with the CLI* (`i5CrTt`, `kjk4x`).
pub(super) fn steps(ctx: &Ctx) -> Element {
  Row::new()
    .id("account-add-steps")
    .width(Dimension::Pct(100.0))
    .spacing(AccountsSpacing::Cell.role())
    .child(step(
      "account-add-step-1",
      Icon::CircleDot,
      &t(ctx, "accounts.add.step1"),
      true,
      None,
    ))
    .child(step(
      "account-add-step-2",
      Icon::Circle,
      &t(ctx, "accounts.add.step2"),
      false,
      None,
    ))
    .into()
}

/// One step: its icon and title, filled while it is the current one, with what was done under a finished one.
pub(super) fn step(id: &str, glyph: Icon, title: &str, current: bool, done: Option<&str>) -> Element {
  let color = if current || done.is_some() {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextMuted
  };
  let icon_color = if done.is_some() {
    PaletteColor::Success
  } else {
    color.clone()
  };
  let row = Row::new()
    .id(id.to_owned())
    .flex(1.0)
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .describe("current", current.to_string())
    .child(crate::ui::icon::icon(glyph, crate::theme::IconSize::Base, icon_color))
    .child(
      Column::new()
        .spacing(AccountsSpacing::Lines.role())
        .child(Text::new(title).variant(ExtraTypography::Strong.role()).color(color))
        .with_children(done.map(|done| Text::new(done).variant(ExtraTypography::Note.role()))),
    );
  if current {
    row
      .background(crate::theme::ExtraColor::Hover.role())
      .rounded(lurq::app::theme::RadiusSize::Md)
      .into()
  } else {
    row.into()
  }
}

fn label(ctx: &Ctx, key: &str) -> Text {
  Text::new(&t(ctx, key))
    .variant(ExtraTypography::Note.role())
    .color(PaletteColor::TextSecondary)
}

fn cli_choice(ctx: &Ctx, draft: &Draft) -> Element {
  let chosen = draft.cli.get().0;
  let card = |cli: AccountCli, detail: &str| {
    let signal = draft.cli.clone();
    let card = OptionCard::new(
      format!("account-add-cli-{}", cli.as_str()),
      (Icon::SquareTerminal, t(ctx, cli_key(cli))),
      t(ctx, detail),
      chosen == cli,
      move || signal.set(CliChoice(cli)),
    );
    let card = if cli.supports_accounts_on(Platform::current()) {
      card
    } else {
      card.unavailable(t(ctx, "accounts.state.unsupported"))
    };
    Column::new().flex(1.0).child(card)
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(label(ctx, "accounts.add.cli"))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(AccountsSpacing::Cell.role())
        .child(card(AccountCli::ClaudeCode, "accounts.add.claudeDetail"))
        .child(card(AccountCli::Codex, "accounts.add.codexDetail")),
    )
    .into()
}

fn name_field(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Element {
  let typed = draft.name.get();
  let hint = if typed.trim().is_empty() {
    t(ctx, "accounts.add.nameHintEmpty")
  } else {
    ta(
      ctx,
      "accounts.add.nameHint",
      [("cli", t(ctx, cli_key(cli))), ("name", typed.trim().to_owned())],
    )
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(label(ctx, "accounts.add.name"))
    .child(FieldInput::new("account-add-name", t(ctx, "accounts.add.name"), draft.name.clone()).render(ctx))
    .child(
      Text::new(&hint)
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

fn folder_choice(ctx: &mut Ctx, draft: &Draft, cli: AccountCli, problem: Option<&Message>) -> Element {
  let workspace = Services::of(ctx).workspace;
  let chosen = draft.folder.get();
  let planned = workspace
    .planned_folder(cli, draft.name.get().trim())
    .map(|folder| folder.display().to_string());
  let new = {
    let signal = draft.folder.clone();
    let card = OptionCard::new(
      "account-add-folder-new",
      (Icon::FolderPlus, t(ctx, "accounts.add.folderNew")),
      planned.clone().unwrap_or_else(|| t(ctx, "accounts.add.noFolderHere")),
      chosen == FolderChoice::New,
      move || signal.set(FolderChoice::New),
    );
    if planned.is_some() {
      card.mono_description()
    } else {
      card.unavailable(t(ctx, "accounts.add.noFolderHere"))
    }
  };
  let existing = {
    let signal = draft.folder.clone();
    OptionCard::new(
      "account-add-folder-existing",
      (Icon::FolderOpen, t(ctx, "accounts.add.folderExisting")),
      ta(
        ctx,
        "accounts.add.folderExistingDetail",
        [
          ("program", cli.program().to_owned()),
          ("variable", cli.folder_variable().to_owned()),
        ],
      ),
      chosen == FolderChoice::Existing,
      move || signal.set(FolderChoice::Existing),
    )
  };
  let path = (chosen == FolderChoice::Existing).then(|| path_field(ctx, draft, problem));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(label(ctx, "accounts.add.folder"))
    .child(new)
    .child(existing)
    .with_children(path)
    .into()
}

/// The brought folder's path, *Choose…*, and why it can't be an account's (`GE7a3`).
fn path_field(ctx: &mut Ctx, draft: &Draft, problem: Option<&Message>) -> Element {
  let pick = draft.pick.clone();
  let field = FieldInput::new("account-add-path", t(ctx, "accounts.add.folder"), draft.path.clone())
    .placeholder(t(ctx, "accounts.add.folderPlaceholder"))
    .mono();
  let field = match problem {
    Some(_) => field.invalid(),
    None => field,
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).child(field.render(ctx)))
        .child(
          ActionButton::new(
            "account-add-choose",
            ButtonKind::Secondary,
            t(ctx, "accounts.add.choose"),
          )
          .icon(Icon::FolderOpen)
          .on_click(move || pick.run(())),
        ),
    )
    .with_children(problem.map(|problem| field_error("account-add-folder-error", &message_text(ctx, problem))))
    .into()
}

/// Why the brought folder can't be an account's, as checked against the accounts and the listed projects.
fn folder_problem(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Option<Message> {
  let path = draft.path.get();
  if draft.folder.get() != FolderChoice::Existing || path.trim().is_empty() {
    return None;
  }
  let workspace = Services::of(ctx).workspace;
  ctx
    // Asked again when the CLI changes too: the account's CLI is part of what the person chose.
    .future((cli, path), move |(_, path)| {
      let workspace = workspace.clone();
      async move {
        let folder = PathBuf::from(path.trim());
        let problem: Option<FolderProblem> = background::run(move || workspace.brought_folder_problem(&folder))
          .await?
          .map_err(|error| orchester_workspace::messages::store_error(&error))?;
        Ok::<_, Message>(problem.map(|problem| AccountsError::Folder(problem).message()))
      }
    })
    .state()
    .get()
    .data
    .flatten()
}

/// *Cancel* and *Create folder and sign in* (or *Add and check* for a brought folder), with what the save said.
fn footer(ctx: &mut Ctx, work: &Work, blocked: bool) -> Element {
  let navigation = Navigation::of(ctx);
  let draft = work.draft.clone();
  let existing = draft.folder.get() == FolderChoice::Existing;
  let name = Name::new(&draft.name.get()).ok();
  let ready = name.is_some() && !blocked && (!existing || !draft.path.get().trim().is_empty());
  let key = if existing {
    "accounts.add.bring"
  } else {
    "accounts.add.create"
  };
  let save = ActionButton::new("account-add-save", ButtonKind::Primary, t(ctx, key)).icon(Icon::ArrowRight);
  let save = if work.change.state().get().is_pending() {
    save.loading()
  } else if let (true, Some(name)) = (ready, name) {
    let change = work.change.clone();
    save.on_click(move || {
      let folder = if draft.folder.get() == FolderChoice::Existing {
        NewFolder::Bring(PathBuf::from(draft.path.get().trim()))
      } else {
        NewFolder::Create
      };
      change.run(Change::Add(NewAccount {
        cli: draft.cli.get().0,
        name: name.clone(),
        folder,
      }));
    })
  } else {
    save.disabled()
  };
  let cancel = ActionButton::new(
    "account-add-cancel",
    ButtonKind::Secondary,
    t(ctx, "accounts.add.cancel"),
  )
  .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::default())));
  let error = work
    .change_error()
    .map(|error| field_error("account-add-error", &message_text(ctx, &error)));
  Row::new()
    .id("account-add-footer")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Panel.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(error)
    .child(Spacer::new().flex(1.0))
    .child(cancel)
    .child(save)
    .into()
}
