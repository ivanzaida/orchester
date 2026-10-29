//! *Add an account* as a page of its own (`03c · Add account` `XAm8i`): the crumb back to Accounts, the title and what
//! an account is, then *1 · Name and folder* — the CLI, the name (with how it will read under it) and the folder, a new
//! one Orchester creates in its application data or one the person already uses, which must stay outside every project
//! and repository — and *2 · Sign in with the CLI*, what comes next. *Create folder and sign in* saves the account and
//! goes on to its sign-in (`sign_in`); a brought folder is checked at once instead.

use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountCli, FolderProblem},
  name::Name,
};
use orchester_workspace::{
  accounts::{AccountsError, NewAccount, NewFolder},
  background,
  messages::Message,
};

use super::{
  Work,
  words::{cli_key, login_command, own_folder_text, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::settings::frame,
  services::Services,
  theme::{ExtraTypography, IconSize, SettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    setting_rows::{SettingGroup, SettingRow},
    tabs::{Segment, SegmentFace, segmented},
    toggle::{Toggle, ToggleKind},
  },
};

/// The name's field (`Input` `t5Yiek`) and the folder's options (`Control` `dysaB`).
const NAME_WIDTH: f32 = 340.0;
const FOLDER_WIDTH: f32 = 460.0;

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
    let first = AccountCli::ALL[0];
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
}

/// The page's crumb and head, which its sign-in step shares.
pub(super) fn heading(ctx: &mut Ctx, pane: Column) -> Column {
  let navigation = Navigation::of(ctx);
  pane
    .child(frame::crumb(
      "account-add-crumb",
      &t(ctx, "settings.tabs.accounts"),
      navigation.to(Route::Accounts(AccountsView::default())),
    ))
    .child(frame::head(
      "account-add-head",
      None,
      (
        &t(ctx, "accounts.add.title"),
        Some(frame::lead("account-add-lead", &t(ctx, "accounts.add.sub")).into()),
      ),
      Vec::new(),
    ))
}

pub(super) fn page(ctx: &mut Ctx, work: &Work) -> Column {
  let draft = &work.draft;
  let cli = draft.cli.get().0;
  let problem = folder_problem(ctx, draft, cli);
  let first = SettingGroup::new("account-add-step-1", t(ctx, "accounts.add.step1"))
    .row(cli_row(ctx, draft, cli))
    .row(name_row(ctx, draft, cli))
    .row(folder_row(ctx, draft, cli, problem.as_ref()));
  let second = SettingGroup::new("account-add-step-2", t(ctx, "accounts.add.step2")).row(after_row(ctx, draft, cli));
  let blocked = problem.is_some();
  heading(ctx, frame::pane("account-add-page"))
    .child(first)
    .child(second)
    .child(footer(ctx, work, blocked))
}

/// *After the folder is made* (or *added*): what comes next, and in its tooltip what the folder is: where Orchester
/// creates a new one and that it starts empty, or that a brought one is never deleted.
fn after_row(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Element {
  let row = if draft.folder.get() == FolderChoice::Existing {
    SettingRow::new("account-add-after", t(ctx, "accounts.add.afterBrought"))
      .hint(t(ctx, "accounts.add.afterBroughtHint"))
      .info(ta(
        ctx,
        "accounts.add.afterBroughtInfo",
        [("variable", cli.folder_variable().to_owned())],
      ))
  } else {
    let row = SettingRow::new("account-add-after", t(ctx, "accounts.add.after")).hint(ta(
      ctx,
      "accounts.add.afterHint",
      [("command", login_command(cli))],
    ));
    match planned_folder(ctx, draft, cli) {
      Some(folder) => row.info(ta(
        ctx,
        "accounts.add.afterInfo",
        [("path", folder), ("folder", own_folder_text(cli))],
      )),
      None => row,
    }
  };
  row.render(ctx)
}

/// *CLI*: Claude Code or Codex.
fn cli_row(ctx: &mut Ctx, draft: &Draft, chosen: AccountCli) -> Element {
  let segments = [AccountCli::ClaudeCode, AccountCli::Codex].map(|cli| {
    let signal = draft.cli.clone();
    Segment::new(
      format!("account-add-cli-{}", cli.as_str()),
      SegmentFace::Text(ctx.t(cli_key(cli))),
      chosen == cli,
      move || signal.set(CliChoice(cli)),
    )
  });
  SettingRow::new("account-add-cli", t(ctx, "accounts.add.cli"))
    .control(segmented("account-add-cli-choice", segments, Focus::Keyboard))
    .render(ctx)
}

/// *Name*: how the account will read under it (`e4MUb`), and the rest of what the name is for in the tooltip.
fn name_row(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Element {
  let typed = draft.name.get();
  let typed = typed.trim();
  let field = FieldInput::new("account-add-name", t(ctx, "accounts.add.name"), draft.name.clone())
    .width(Dimension::Px(NAME_WIDTH))
    .render(ctx);
  let row = SettingRow::new("account-add-name-row", t(ctx, "accounts.add.name")).control(field);
  let cli = t(ctx, cli_key(cli));
  let row = if typed.is_empty() {
    row
      .hint(ta(ctx, "accounts.add.nameShownEmpty", [("cli", cli)]))
      .info(t(ctx, "accounts.add.nameHintEmpty"))
  } else {
    row
      .hint(ta(
        ctx,
        "accounts.add.nameShown",
        [("cli", cli.clone()), ("name", typed.to_owned())],
      ))
      .info(ta(
        ctx,
        "accounts.add.nameHint",
        [("cli", cli), ("name", typed.to_owned())],
      ))
  };
  row.render(ctx)
}

/// The folder Orchester would create for a new account of `cli` named as typed so far; none on a computer where it has
/// no folder for new accounts.
fn planned_folder(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Option<String> {
  Services::of(ctx)
    .workspace
    .planned_folder(cli, draft.name.get().trim())
    .map(|folder| folder.display().to_string())
}

/// *Folder*: a new one Orchester creates or one the person already uses, and for that one its path, *Choose…* and
/// why it can't be an account's. The options say what the folder is, not its path (`gw62M`, `zJwuL`); the path of a
/// new one is in *After the folder is made*'s tooltip.
fn folder_row(ctx: &mut Ctx, draft: &Draft, cli: AccountCli, problem: Option<&Message>) -> Element {
  let chosen = draft.folder.get();
  let planned = planned_folder(ctx, draft, cli).is_some();
  let new_detail = if planned {
    t(ctx, "accounts.add.folderNewDetail")
  } else {
    t(ctx, "accounts.add.noFolderHere")
  };
  let new = folder_option(
    ("account-add-folder-new", &t(ctx, "accounts.add.folderNew")),
    &new_detail,
    chosen == FolderChoice::New,
    planned
      .then(|| draft.folder.clone())
      .map(|signal| move || signal.set(FolderChoice::New)),
  );
  let existing_detail = ta(
    ctx,
    "accounts.add.folderExistingDetail",
    [("program", cli.program().to_owned())],
  );
  let signal = draft.folder.clone();
  let existing = folder_option(
    ("account-add-folder-existing", &t(ctx, "accounts.add.folderExisting")),
    &existing_detail,
    chosen == FolderChoice::Existing,
    Some(move || signal.set(FolderChoice::Existing)),
  );
  let options = Column::new()
    .id("account-add-folder-options")
    .width(Dimension::Px(FOLDER_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(new)
    .child(existing);
  let row = SettingRow::new("account-add-folder", t(ctx, "accounts.add.folder")).control(options);
  let row = if chosen == FolderChoice::Existing {
    row.below(path_field(ctx, draft, problem))
  } else {
    row
  };
  row.render(ctx)
}

/// An option of *Folder*: its radio, its name and what it means; without `choose` it can't be chosen here.
fn folder_option(
  (id, title): (&str, &str),
  detail: &str,
  chosen: bool,
  choose: Option<impl Fn() + Send + Sync + 'static>,
) -> Element {
  let radio = Toggle::new(id.to_owned(), ToggleKind::Radio, chosen, title.to_owned()).hide_label();
  let radio = match choose {
    Some(choose) => radio.on_change(move |_| choose()),
    None => radio,
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(radio)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(
          Text::new(title)
            .id(format!("{id}-title"))
            .variant(SettingsText::OptionTitle.role()),
        )
        .child(
          Text::new(detail)
            .id(format!("{id}-detail"))
            .variant(ExtraTypography::Note.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The brought folder's path, *Choose…*, and why it can't be an account's.
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

/// Why account folders stay outside projects, *Cancel* and *Create folder and sign in* (or *Add and check* for a
/// brought folder), with what the save said over them (`Actions` `CJzgg`).
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
  let cancel = ActionButton::new("account-add-cancel", ButtonKind::Ghost, t(ctx, "accounts.add.cancel"))
    .icon(Icon::X)
    .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::default())));
  let error = work
    .change_error()
    .map(|error| field_error("account-add-error", &message_text(ctx, &error)));
  let reason = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&t(ctx, "accounts.add.folderNote"))
        .id("account-add-note")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(error)
    .child(
      Row::new()
        .id("account-add-footer")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(reason)
        .child(cancel)
        .child(save),
    )
    .into()
}
