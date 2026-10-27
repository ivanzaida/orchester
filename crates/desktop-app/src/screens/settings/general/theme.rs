//! *Theme* (`Theme` `IcZES` in `01 · General`): System, Dark or Light in a segmented control with their glyphs
//! (`Segmented` `IxNCM`), what the theme does in the row's tooltip, and, when a choice could not be kept, that nothing
//! changed, under the row. The honest note that System keeps the appearance the operating system had when Orchester
//! started is under the group ([`NOTE`]). The choice is kept off the UI thread through the window's [`Appearance`],
//! which `set_theme` shares, one at a time and the newest last; the window takes its colors once one is kept.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use crate::{
  appearance::{Appearance, AppearanceError, ThemeSetting},
  screens::settings::{save_state::SaveState, serial::Serial},
  theme::Mode,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    pressable::Focus,
    setting_rows::SettingRow,
    tabs::{Segment, SegmentFace, segmented},
  },
};

const ID: &str = "settings-theme";

/// A theme that was kept, with the mode it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kept(ThemeSetting, Mode);

impl DevtoolsInspectable for Kept {}

/// Why a theme was not kept: nothing changed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum NotKept {
  /// The window has no application database to keep it in.
  NoDatabase,
  /// The application database did not take it.
  NotSaved(Message),
}

impl DevtoolsInspectable for NotKept {}

/// The theme last pressed, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Chosen(Option<ThemeSetting>);

impl DevtoolsInspectable for Chosen {}

type Keeps = Serial<ThemeSetting, Kept, NotKept>;

pub struct ThemeRow {
  /// The window's colors, which a window without one cannot change here.
  appearance: Option<Appearance>,
  keep: Keeps,
  /// The choice being kept, shown chosen until it is.
  chosen: Signal<Chosen>,
}

impl Component for ThemeRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let appearance = ctx.use_context::<Appearance>();
    let keeper = appearance.clone();
    let action: FutureAction<ThemeSetting, Kept, NotKept> = ctx.future_action(move |setting: ThemeSetting| {
      let appearance = keeper.clone();
      async move {
        let appearance = appearance.ok_or(NotKept::NoDatabase)?;
        background::run(move || appearance.keep(setting))
          .await
          .map_err(NotKept::NotSaved)?
          .map(|mode| Kept(setting, mode))
          .map_err(|error| {
            tracing::error!(%error, "the theme was not kept");
            match error {
              AppearanceError::NoSettings => NotKept::NoDatabase,
              AppearanceError::NotSaved(error) => NotKept::NotSaved(messages::store_error(&error)),
            }
          })
      }
    });
    let keep = Serial::new(ctx, action);
    // Kept off the UI thread, shown on it: the window takes the mode as soon as the choice is in the database.
    let (shower, theme) = (appearance.clone(), ctx.theme().clone());
    ctx.watch(&keep.state(), move |state| {
      let done = !state.is_pending() && state.error.is_none();
      if let (Some(appearance), Some(Kept(setting, mode)), true) = (&shower, state.data, done) {
        appearance.show(&theme, setting, mode);
      }
    });
    Self {
      appearance,
      keep,
      chosen: ctx.signal(Chosen(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Reading the theme follows its changes, so a theme `set_theme` chose shows here too.
    ctx.theme();
    let state = self.keep.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, self.keep.outcome(&state));
    }
    let busy = self.keep.busy(&state);
    let current = self.appearance.as_ref().map(Appearance::setting);
    let shown = match self.chosen.get() {
      Chosen(Some(chosen)) if busy => Some(chosen),
      _ => current,
    };
    let control = self.control(ctx, shown);
    let problem = match (&state.error, &self.appearance) {
      (_, None) => Some(ctx.t("settings.general.theme.unavailable").to_string()),
      (Some(NotKept::NoDatabase), _) if !busy => Some(ctx.t("settings.general.theme.noDatabase").to_string()),
      (Some(NotKept::NotSaved(reason)), _) if !busy => {
        let reason = message_text(ctx, reason);
        Some(
          ctx
            .t_args("settings.general.theme.notSaved", [("reason", &*reason)])
            .to_string(),
        )
      }
      _ => None,
    };
    SettingRow::new(ID, ctx.t("settings.general.theme.title"))
      .info(ctx.t(DESCRIPTION))
      .control(control)
      .below_all(problem.map(|problem| field_error(&format!("{ID}-error"), &problem).into()))
      .render(ctx)
  }
}

impl ThemeRow {
  /// System · Dark · Light, `shown` chosen; a press keeps the theme it names (`Segmented` `IxNCM`).
  fn control(&self, ctx: &Ctx, shown: Option<ThemeSetting>) -> Element {
    let segments = [ThemeSetting::System, ThemeSetting::Dark, ThemeSetting::Light].map(|setting| {
      let (keep, chosen) = (self.keep.clone(), self.chosen.clone());
      let (key, glyph) = match setting {
        ThemeSetting::Dark => ("settings.general.theme.dark", Icon::Moon),
        ThemeSetting::Light => ("settings.general.theme.light", Icon::Sun),
        ThemeSetting::System => ("settings.general.theme.system", Icon::Monitor),
      };
      let label: Arc<str> = ctx.t(key);
      Segment::new(
        format!("{ID}-{}", setting.name()),
        SegmentFace::Labelled(glyph, label),
        shown == Some(setting),
        move || {
          chosen.set(Chosen(Some(setting)));
          keep.save(setting);
        },
      )
    });
    segmented(&format!("{ID}-choice"), segments, Focus::Keyboard).into()
  }
}

/// What the row says the theme does, in the operating system's name.
#[cfg(target_os = "macos")]
const DESCRIPTION: &str = "settings.general.theme.descriptionMacos";
#[cfg(not(target_os = "macos"))]
const DESCRIPTION: &str = "settings.general.theme.descriptionWindows";
/// The honest note under *Appearance*: System follows only what the operating system had when Orchester started.
#[cfg(target_os = "macos")]
pub(super) const NOTE: &str = "settings.general.theme.noteMacos";
#[cfg(not(target_os = "macos"))]
pub(super) const NOTE: &str = "settings.general.theme.noteWindows";
