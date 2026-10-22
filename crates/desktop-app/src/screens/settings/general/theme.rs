//! *Theme* (`Row · Theme` `BDAwf` in `hUaXB`): Dark, Light or System in a segmented control (`Segmented/text`
//! `S5Fsve`), the honest note that System keeps the appearance the operating system had when Orchester started
//! (`Honest note` `A9YKqx`), and, when a choice could not be kept, that nothing changed (`DygwC` in `g9i0n` of
//! `HNQBR`). The choice is kept off the UI thread through the window's [`Appearance`], which `set_theme` shares; the
//! window takes its colors once it is kept.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use crate::{
  appearance::{Appearance, AppearanceError, ThemeSetting},
  screens::settings::save_state::{Outcome, SaveState},
  theme::Mode,
  ui::{
    field::field_error,
    form_section::{Label, Place, side_row},
    icon::Icon,
    inspector::note,
    message::message_text,
    pressable::Focus,
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

type KeepAction = FutureAction<ThemeSetting, Kept, NotKept>;

pub struct ThemeRow {
  /// The window's colors, which a window without one cannot change here.
  appearance: Option<Appearance>,
  keep: KeepAction,
  /// The choice being kept, shown chosen until it is.
  chosen: Signal<Chosen>,
}

impl Component for ThemeRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let appearance = ctx.use_context::<Appearance>();
    let keeper = appearance.clone();
    let keep: KeepAction = ctx.future_action(move |setting: ThemeSetting| {
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
      saves.report(ID, Outcome::of(&state));
    }
    let current = self.appearance.as_ref().map(Appearance::setting);
    let shown = match self.chosen.get() {
      Chosen(Some(chosen)) if state.is_pending() => Some(chosen),
      _ => current,
    };
    let control = self.control(ctx, shown);
    let row = side_row(
      Label::new(ID, ctx.t("settings.general.theme.title")).description(ctx.t(words::DESCRIPTION)),
      control,
      Place::Within,
      false,
    );
    let problem = match (&state.error, &self.appearance) {
      (_, None) => Some(ctx.t("settings.general.theme.unavailable").to_string()),
      (Some(NotKept::NoDatabase), _) if !state.is_pending() => {
        Some(ctx.t("settings.general.theme.noDatabase").to_string())
      }
      (Some(NotKept::NotSaved(reason)), _) if !state.is_pending() => {
        let reason = message_text(ctx, reason);
        Some(
          ctx
            .t_args("settings.general.theme.notSaved", [("reason", &*reason)])
            .to_string(),
        )
      }
      _ => None,
    };
    Column::new().width(Dimension::Pct(100.0)).child(row).child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_bottom(SpacingSize::Xs)
        .spacing(SpacingSize::Sm)
        .child(note(&format!("{ID}-note"), Icon::Info, &ctx.t(words::NOTE)))
        .with_children(problem.map(|problem| field_error(&format!("{ID}-error"), &problem))),
    )
  }
}

impl ThemeRow {
  /// Dark · Light · System, `shown` chosen; a press keeps the theme it names.
  fn control(&self, ctx: &Ctx, shown: Option<ThemeSetting>) -> Element {
    let segments = ThemeSetting::ALL.map(|setting| {
      let (keep, chosen) = (self.keep.clone(), self.chosen.clone());
      let label: Arc<str> = ctx.t(match setting {
        ThemeSetting::Dark => "settings.general.theme.dark",
        ThemeSetting::Light => "settings.general.theme.light",
        ThemeSetting::System => "settings.general.theme.system",
      });
      Segment::new(
        format!("{ID}-{}", setting.name()),
        SegmentFace::Text(label),
        shown == Some(setting),
        move || {
          chosen.set(Chosen(Some(setting)));
          keep.run(setting);
        },
      )
    });
    segmented(&format!("{ID}-choice"), segments, Focus::Keyboard).into()
  }
}

/// The words that name the operating system.
mod words {
  #[cfg(target_os = "macos")]
  pub const DESCRIPTION: &str = "settings.general.theme.descriptionMacos";
  #[cfg(not(target_os = "macos"))]
  pub const DESCRIPTION: &str = "settings.general.theme.descriptionWindows";
  #[cfg(target_os = "macos")]
  pub const NOTE: &str = "settings.general.theme.noteMacos";
  #[cfg(not(target_os = "macos"))]
  pub const NOTE: &str = "settings.general.theme.noteWindows";
}
