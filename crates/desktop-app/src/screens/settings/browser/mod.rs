//! Settings › Browser (`09 · Settings · Browser` `j3Lre` and `09b · … not found` `NZECi` in `Browser · redesign`
//! `lcNez` of the new `orchester.pen`; `docs/design/browser.md`, "Finding the browser"). *Browser program*
//! ([`rows`]) says which Chromium-based browser agents' pages open in: the one found on this computer, or the program
//! the person chose with *Choose another program…*, which is refused under its row, with the reason, when it is not a
//! Chromium-based browser's. None found, a notice over the rows says what was looked for, with *Look again*, and the
//! row says agents' browser tools answer that the browser is unavailable. Under them is the folder of the projects'
//! browser profiles with Copy. *Browser window* ([`window`]) holds *Open the Browser window when an agent starts
//! browsing*, on until a person turns it off.
//!
//! What it shows is read off the UI thread through its query (finding the browser reads the registry on Windows),
//! which the window read as the session started: the last read shows at once and the browser is looked for again
//! behind it each time the tab shows, after each choice and at *Look again*; the runner starts a project's browser by
//! the same rule. Until the first read ever ends the groups are a skeleton ([`reading`], `L12L` `vC4Jf`). The header's
//! save state follows the choice and the switch.

mod reading;
mod rows;
mod window;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_exec::browser::{BrowserProgram, NotABrowser};
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::ChooseBrowserError,
};

use super::{
  frame,
  save_state::{Outcome, SaveState},
};
use crate::{
  queries::{self, Again, Session, app},
  services::path_picker::{PathKind, PathPicker},
  theme::SettingsSpacing,
  ui::{message::message_text, state::load_error},
};

/// The tab, scrolling under the header.
pub fn tab(ctx: &mut Ctx) -> Element {
  let pane = frame::pane("settings-browser").child(ctx.mount_keyed::<BrowserSection>("browser", ()));
  frame::column("settings-browser-scroll", pane)
}

/// What came of *Choose another program…*.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Choice {
  /// The picker closed without a program.
  Closed,
  /// The program is kept as agents' browser.
  Taken(BrowserProgram),
  /// The program is not a Chromium-based browser's: nothing changed.
  Refused(NotABrowser),
}

impl DevtoolsInspectable for Choice {}

type ChooseAction = FutureAction<(), Choice, Message>;

/// Both groups, once what they show was ever read.
struct BrowserSection {
  /// Counts the times *Look again* (or *Try again*) asked for a new read.
  revision: Signal<u64>,
  again: Again,
}

impl Component for BrowserSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
      again: Again::live(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let session = Session::of(ctx);
    // Hooks in the same order on every render: the choice, then the read, which each choice reads again.
    let choose = choose_action(ctx, &session);
    let chosen = choose.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-browser-program", Outcome::of(&chosen));
    }
    // Through its query, which the window read as the session started: the last read at once, and the browser looked
    // for again behind it each time the tab shows, at *Look again* and after each choice (`queries`).
    let query = ctx.query(app::browser_settings(session));
    self.again.follow(self.revision.get(), &query);
    let read = queries::state(&query);
    let revision = self.revision.clone();
    let again = move || revision.update(|revision| *revision += 1);
    let settings = match (&read.data, &read.error) {
      (_, Some(error)) => return failed(ctx, error, again),
      (Some(settings), _) => Arc::clone(settings),
      _ => return reading::skeleton(ctx),
    };
    let actions = rows::Actions {
      choose: &choose,
      chosen: &chosen,
      looking: query.fetching(),
      again: Arc::new(again),
    };
    Column::new()
      .id("settings-browser-groups")
      .width(Dimension::Pct(100.0))
      .spacing(SettingsSpacing::Page.role())
      .child(rows::program_group(ctx, &settings, &actions))
      .child(ctx.mount_keyed::<window::WindowSwitch>("browser-window", settings.opens_window))
      .into()
  }
}

/// Why Settings › Browser cannot be read, with *Try again*.
fn failed(ctx: &Ctx, error: &Message, again: impl Fn() + Send + Sync + 'static) -> Element {
  load_error(
    ctx,
    "settings-browser-load-error",
    (
      &ctx.t("settings.browser.readFailed"),
      &ctx.t_args(
        "settings.browser.readFailureDetail",
        [("reason", message_text(ctx, error))],
      ),
    ),
    again,
  )
}

/// Asks for a program with the system's picker and keeps it when it is a Chromium-based browser's; the workspace
/// counts a program kept as a change, which reads Settings › Browser again (`queries`).
fn choose_action(ctx: &mut Ctx, Session(services): &Session) -> ChooseAction {
  let picker = PathPicker::of(ctx);
  let title = ctx.t("settings.browser.program.pickerTitle");
  let workspace = services.workspace.clone();
  ctx.future_action(move |()| {
    let (picker, title, workspace) = (picker.clone(), title.clone(), workspace.clone());
    async move {
      let Some(program) = picker.pick(PathKind::File, title).await else {
        return Ok(Choice::Closed);
      };
      match background::run(move || workspace.choose_browser_program(&program)).await? {
        Ok(taken) => Ok(Choice::Taken(taken)),
        Err(ChooseBrowserError::NotABrowser(reason)) => Ok(Choice::Refused(reason)),
        Err(ChooseBrowserError::Store(error)) => {
          tracing::error!(%error, "the browser program chosen was not saved");
          Err(messages::store_error(&error))
        }
      }
    }
  })
}
