//! Settings › Browser (`09 · Settings · Browser` `j3Lre` and `09b · … not found` `NZECi` in `Browser · redesign`
//! `lcNez` of the new `orchester.pen`; `docs/design/browser.md`, "Finding the browser"). *Browser program*
//! ([`rows`]) says which Chromium-based browser agents' pages open in: the one found on this computer, or the program
//! the person chose with *Choose another program…*, which is refused under its row, with the reason, when it is not a
//! Chromium-based browser's. None found, a notice over the rows says what was looked for, with *Look again*, and the
//! row says agents' browser tools answer that the browser is unavailable. Under them is the folder of the projects'
//! browser profiles with Copy. *Browser window* ([`window`]) holds *Open the Browser window when an agent starts
//! browsing*, on until a person turns it off; the window is not in this version yet, as the group's note says.
//!
//! What it shows is read off the UI thread (finding the browser reads the registry on Windows), again after each
//! choice and at *Look again*; the runner starts a project's browser by the same rule. The header's save state
//! follows the choice and the switch.

mod rows;
mod window;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_exec::browser::{BrowserProgram, NotABrowser};
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::{BrowserSettings, ChooseBrowserError, Workspace},
};

use super::{
  frame,
  save_state::{Outcome, SaveState},
};
use crate::{
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  theme::SettingsSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

/// The tab, scrolling under the header.
pub fn tab(ctx: &mut Ctx) -> Element {
  let pane = frame::pane("settings-browser").child(ctx.mount_keyed::<BrowserSection>("browser", ()));
  frame::column("settings-browser-scroll", pane)
}

/// What Settings › Browser shows, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(Arc<BrowserSettings>);

impl DevtoolsInspectable for Read {}

/// What came of *Choose another program…*. Each program taken is a different choice, so the section is read again
/// after each, the second of two in a row too.
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

/// Both groups, once what they show is read.
struct BrowserSection {
  /// Counts the times *Look again* (or *Try again*) asked for a new read.
  revision: Signal<u64>,
}

impl Component for BrowserSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = Services::of(ctx).workspace;
    // Hooks in the same order on every render: the choice, then the read, which follows each choice.
    let choose = choose_action(ctx, &workspace);
    let chosen = choose.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-browser-program", Outcome::of(&chosen));
    }
    let read = read(ctx, &workspace, (self.revision.get(), chosen.data.clone()));
    let revision = self.revision.clone();
    let again = move || revision.update(|revision| *revision += 1);
    let settings = match (&read.data, &read.error) {
      (_, Some(error)) if !read.is_pending() => return failed(ctx, error, again),
      (Some(settings), _) => Arc::clone(&settings.0),
      _ => return loading("settings-browser-reading", &ctx.t("settings.browser.reading")).into(),
    };
    let actions = rows::Actions {
      choose: &choose,
      chosen: &chosen,
      looking: read.is_pending(),
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

/// Asks for a program with the system's picker and keeps it when it is a Chromium-based browser's.
fn choose_action(ctx: &mut Ctx, workspace: &Workspace) -> ChooseAction {
  let picker = PathPicker::of(ctx);
  let title = ctx.t("settings.browser.program.pickerTitle");
  let workspace = workspace.clone();
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

/// What Settings › Browser shows, read again at each of `deps`: *Look again* and each choice.
fn read(ctx: &mut Ctx, workspace: &Workspace, deps: (u64, Option<Choice>)) -> FutureState<Read, Message> {
  let workspace = workspace.clone();
  ctx
    .future(deps, move |_| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.browser_settings())
          .await?
          .map(|settings| Read(Arc::new(settings)))
          .map_err(|error| {
            tracing::error!(%error, "Settings › Browser cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
}
