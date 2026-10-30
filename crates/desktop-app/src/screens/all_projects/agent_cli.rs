//! The warning that an agent CLI is not found (`SIbdo` and `U4QvYO` in frame 14 `mruEH`; `docs/design/startup.md`,
//! decision 9): the callout naming the CLI, then *Choose the claude file…*, *Check again* and the folders searched, 32
//! under it. It shows on the welcome and above the list, never on *Usage* or when the list cannot be read, and whatever
//! it says a project row still opens. The rule is `orchester_workspace::connections::missing_cli`'s; the file chosen
//! goes through `choose_program`. Read off the UI thread; the last read stays on screen while it is read again. A read
//! that fails is logged and warns of nothing.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_exec::search_path::SearchBasis;
use orchester_workspace::{
  background,
  connections::{
    choose_program::{ChooseProgramError, choose_program},
    missing_cli::{MissingCli, missing_cli_on},
  },
  messages::Message,
  project_list::Listing,
  tasks::outside::{Outside, OutsideChanges},
  workspace::ProjectEntry,
};
use parking_lot::Mutex;

use super::page::error_line;
use crate::{
  services::{
    Services,
    cli_search::CliSearch,
    path_picker::{PathKind, PathPicker},
  },
  theme::{ExtraSpacing, ProjectsText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
    message::message_text,
  },
};

/// What a read found, and the *Check again* it answers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Found {
  missing: Option<MissingCli>,
  checks: u64,
}

impl DevtoolsInspectable for Found {}

/// What choosing a file came to.
#[derive(Clone, Debug, PartialEq)]
enum Chosen {
  /// The picker was closed without a file.
  Closed,
  Fixed,
  /// Why the file was refused, or a connection was not changed.
  Refused(Message),
}

impl DevtoolsInspectable for Chosen {}

/// The picker's title, and the warning the file answers.
type ChooseAction = FutureAction<(Arc<str>, MissingCli), Chosen, Message>;

/// The warning's state on the screen.
pub(super) struct AgentCli {
  /// Counts *Check again*, which reads the login shell again.
  checks: Signal<u64>,
  /// Counts the files chosen, so the warning is read again after one.
  chosen: Signal<u64>,
  /// The *Check again* the login shell was last read for.
  last_check: Arc<Mutex<u64>>,
  /// Whether the choosing's end is followed already; once per screen.
  followed: Mutex<bool>,
}

/// What [`AgentCli::read`] found.
pub(super) struct Read {
  missing: Option<MissingCli>,
  /// Whether a *Check again* is under way.
  checking: bool,
  choose: ChooseAction,
}

/// The entries the warning is read for: none listed on the welcome; `None`, reading nothing, until the list is read and
/// while it cannot be read.
pub(super) fn listed(listing: Option<&Listing>, failed: bool) -> Option<&[ProjectEntry]> {
  match listing {
    _ if failed => None,
    Some(Listing::Projects(entries)) => Some(entries),
    Some(Listing::Welcome) => Some(&[]),
    None => None,
  }
}

impl AgentCli {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      checks: ctx.signal(0),
      chosen: ctx.signal(0),
      last_check: Arc::default(),
      followed: Mutex::new(false),
    }
  }

  /// The warning for `entries`. The screen asks for it in every render, in the same place: lurq keeps a component's
  /// futures by the order it asks for them.
  pub fn read(&self, ctx: &mut Ctx, services: &Services, entries: Option<&[ProjectEntry]>) -> Read {
    let found = self.found(ctx, services, entries);
    let choose = choose_action(ctx, services);
    self.follow(ctx, &choose);
    let checks = self.checks.get();
    let checking = found.is_pending() && found.data.as_ref().map_or(0, |found| found.checks) != checks;
    Read {
      missing: found.data.and_then(|found| found.missing),
      checking,
      choose,
    }
  }

  fn found(
    &self,
    ctx: &mut Ctx,
    services: &Services,
    entries: Option<&[ProjectEntry]>,
  ) -> lurq::app::ctx::FutureState<Found, ()> {
    let (search, last_check) = (CliSearch::of(ctx), Arc::clone(&self.last_check));
    let key = (
      services.workspace.clone(),
      entries.map(<[ProjectEntry]>::to_vec),
      self.checks.get(),
      self.chosen.get(),
    );
    ctx
      .future(key, move |(workspace, entries, checks, _)| {
        let (search, last_check) = (search.clone(), Arc::clone(&last_check));
        async move {
          let Some(entries) = entries else {
            return Ok(Found::default());
          };
          let read = background::run(move || {
            let read_again = std::mem::replace(&mut *last_check.lock(), checks) != checks;
            let (path, basis) = search.search(read_again);
            missing_cli_on(&workspace, &entries, &path, &basis)
          })
          .await;
          match read {
            Ok(Ok(missing)) => Ok(Found { missing, checks }),
            Ok(Err(error)) => {
              tracing::warn!(%error, "cannot read whether an agent CLI is missing");
              Err(())
            }
            // The work did not finish; `background::run` logged why.
            Err(_) => Err(()),
          }
        }
      })
      .state()
      .get()
  }

  /// A file that changed something reads the warning again. One that changed nothing does not need it, but a project
  /// that failed leaves others fixed, so every answer but a closed picker reads it.
  fn follow(&self, ctx: &mut Ctx, choose: &ChooseAction) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let chosen = self.chosen.clone();
    ctx.watch(&choose.state(), move |state| {
      if state.is_pending() || !state.is_fulfilled() {
        return;
      }
      if matches!(state.data, Some(Chosen::Fixed | Chosen::Refused(_))) {
        chosen.update(|count| *count += 1);
      }
    });
  }

  /// The callout, the fix row under it and why the last file was refused; `None` while nothing is missing.
  pub fn element(&self, ctx: &Ctx, read: &Read) -> Option<Element> {
    let missing = read.missing.as_ref()?;
    let (title, body) = words(ctx, missing);
    let fix = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(self.fix_row(ctx, read, missing))
      .with_children(failure(ctx, &read.choose).map(|failure| error_line("start-cli-error", &failure)));
    Some(
      Column::new()
        .id("start-cli")
        .describe("program", missing.program)
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Section)
        .child(
          Callout::new("start-cli-callout", CalloutTone::Warning, title)
            .icon(Icon::Terminal)
            .body(body),
        )
        .child(fix)
        .into(),
    )
  }

  /// *Choose the claude file…*, *Check again* and the folders searched (`U4QvYO`), each button loading during its work.
  fn fix_row(&self, ctx: &Ctx, read: &Read, missing: &MissingCli) -> Row {
    let program = missing.program;
    let choose = ActionButton::new(
      "start-cli-choose",
      ButtonKind::Secondary,
      ctx.t_args("startup.agentCli.choose", [("program", program)]),
    )
    .icon(Icon::FileSearch);
    let choose = if read.choose.state().get().is_pending() {
      choose.loading()
    } else {
      let (action, picked) = (read.choose.clone(), missing.clone());
      let title = ctx.t_args("startup.agentCli.chooseTitle", [("program", program)]);
      choose.on_click(move || action.run((title.clone(), picked.clone())))
    };
    let check = ActionButton::new(
      "start-cli-check",
      ButtonKind::Ghost,
      ctx.t("startup.agentCli.checkAgain"),
    )
    .icon(Icon::RefreshCw);
    let check = if read.checking {
      check.loading()
    } else {
      let checks = self.checks.clone();
      check.on_click(move || checks.update(|count| *count += 1))
    };
    Row::new()
      .id("start-cli-fix")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(choose)
      .child(check)
      .child(searched(ctx, &missing.searched))
  }
}

/// The folders searched, each a text of its own: one text would break inside a path, after its `:` or a `/`. A folder
/// wider than a whole line still breaks. The room after each is padding, since a gap would also space the lines.
fn searched(ctx: &Ctx, folders: &[String]) -> Row {
  let piece = |index: usize, text: &str| {
    Text::new(text)
      .id(format!("start-cli-searched-{index}"))
      .variant(ProjectsText::SearchedFolders.role())
      .min_width(Dimension::Px(0.0))
      .padding(Padding::new().right(ExtraSpacing::Snug.role()))
  };
  let row = Row::new()
    .id("start-cli-searched")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .wrap();
  let Some((last, others)) = folders.split_last() else {
    return row.child(piece(0, &ctx.t("startup.agentCli.noFolders")));
  };
  row
    .child(piece(0, &ctx.t("startup.agentCli.lookedIn")))
    .with_children(others.iter().enumerate().map(|(index, folder)| {
      piece(
        index + 1,
        &ctx.t_args("startup.agentCli.folderInList", [("folder", folder.as_str())]),
      )
    }))
    .child(piece(folders.len(), last))
}

/// The title and what the callout says, never claiming a read of the login shell that did not happen.
fn words(ctx: &Ctx, missing: &MissingCli) -> (Arc<str>, Arc<str>) {
  let program = missing.program;
  let title = |key: &str| ctx.t_args(key, [("program", program)]);
  match &missing.basis {
    SearchBasis::Path => (
      title("startup.agentCli.title"),
      ctx.t_args("startup.agentCli.bodyPath", [("program", program)]),
    ),
    SearchBasis::LoginShell { shell } => (
      title("startup.agentCli.titleMac"),
      ctx.t_args(
        "startup.agentCli.bodyLoginShell",
        [("program", program), ("shell", shell.as_str())],
      ),
    ),
    SearchBasis::UsualFolders => (
      title("startup.agentCli.titleMac"),
      ctx.t_args("startup.agentCli.bodyUsualFolders", [("program", program)]),
    ),
  }
}

fn choose_action(ctx: &mut Ctx, services: &Services) -> ChooseAction {
  let picker = PathPicker::of(ctx);
  let (workspace, changes) = (services.workspace.clone(), services.changes.clone());
  ctx.future_action(move |(title, missing): (Arc<str>, MissingCli)| {
    let (picker, workspace, changes) = (picker.clone(), workspace.clone(), changes.clone());
    async move {
      let Some(file) = picker.pick(PathKind::File, title).await else {
        return Ok(Chosen::Closed);
      };
      let chosen = background::run(move || choose_program(&workspace, &missing, &file)).await?;
      Ok(match chosen {
        Ok(fixed) => {
          record(&changes, &fixed);
          Chosen::Fixed
        }
        Err(error) => {
          tracing::warn!(%error, "the file chosen for an agent CLI was not taken");
          if let ChooseProgramError::NotSaved { fixed, .. } = &error {
            record(&changes, fixed);
          }
          Chosen::Refused(error.message())
        }
      })
    }
  })
}

/// Tells the screens that show the connections of `fixed` that they changed.
fn record(changes: &OutsideChanges, fixed: &[ProjectId]) {
  for project in fixed {
    changes.record(*project, Outside::Connections);
  }
}

/// Why the last file was not taken; `None` once one was, while the next attempt runs, and before the first.
fn failure(ctx: &Ctx, choose: &ChooseAction) -> Option<Arc<str>> {
  let state = choose.state().get();
  if state.is_pending() {
    return None;
  }
  // Choosing stopped before it ended.
  if let Some(reason) = state.error.as_ref().filter(|_| state.is_rejected()) {
    return Some(message_text(ctx, reason));
  }
  match state.data {
    Some(Chosen::Refused(reason)) => Some(message_text(ctx, &reason)),
    _ => None,
  }
}
