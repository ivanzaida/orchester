//! Settings › General (`Settings · General · dark` `hUaXB`): *Appearance* with the window's theme (`theme`), *Plan
//! limits* with the warning threshold every project's Usage draws (`threshold`; the frame draws it as planned, but the
//! Usage dashboard is built and the person chose a working field here, 2026-10-22), and *This computer*: the app's
//! version and build, and where its application database and its log are, each with Copy. A change saves as it is
//! made, off the UI thread, and the header's save state follows it (`super::save_state`). Nothing here widens what
//! agents may do, so no control is a person's alone.

mod theme;
mod threshold;

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{app_paths, background};

use crate::{
  services::Services,
  theme::ExtraTypography,
  ui::{
    command_row::CommandRow,
    field::field_error,
    form_section::{Label, Place, section, side_row, stacked_row},
    settings_page::form_scroll,
  },
};

/// The tab, scrolling under the header.
pub fn tab(ctx: &mut Ctx) -> Element {
  let form = Column::new()
    .id("settings-general")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xl)
    .child(section(
      "settings-appearance",
      &ctx.t("settings.general.appearance"),
      None,
      vec![ctx.mount_keyed::<theme::ThemeRow>("settings-theme", ())],
    ))
    .child(section(
      "settings-plan-limits",
      &ctx.t("settings.general.planLimits"),
      None,
      vec![ctx.mount_keyed::<threshold::ThresholdRow>("settings-threshold", ())],
    ))
    .child(ctx.mount_keyed::<ThisComputer>("settings-computer", ()));
  form_scroll("settings-general-scroll", form).into()
}

/// Where the application database and the log are, as read off the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Places {
  database: Option<PathBuf>,
  log: Option<PathBuf>,
}

impl DevtoolsInspectable for Places {}

/// *This computer*: the version, the application data and the app log.
struct ThisComputer;

impl Component for ThisComputer {
  type Props = ();

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let workspace = Services::of(ctx).workspace;
    let places = ctx
      .future((), move |()| {
        let workspace = workspace.clone();
        async move {
          background::run(move || Places {
            database: workspace.database_path(),
            log: app_paths::log_dir()
              .inspect_err(|error| tracing::warn!(%error, "the application log folder is unknown"))
              .ok(),
          })
          .await
        }
      })
      .state()
      .get()
      .data;
    let data = PlaceKeys {
      id: "settings-data",
      title: "settings.general.data.title",
      description: "settings.general.data.description",
      unknown: "settings.general.data.unknown",
    };
    let log_keys = PlaceKeys {
      id: "settings-log",
      title: "settings.general.log.title",
      description: "settings.general.log.description",
      unknown: "settings.general.log.unknown",
    };
    let rows = vec![
      version(ctx),
      place(ctx, &data, places.as_ref().map(|places| places.database.as_ref())),
      place(ctx, &log_keys, places.as_ref().map(|places| places.log.as_ref())),
    ];
    section("settings-computer", &ctx.t("settings.general.computer"), None, rows)
  }
}

/// *Version*: what this build is, and its version in Geist Mono at the row's end.
fn version(ctx: &Ctx) -> Element {
  let version = env!("CARGO_PKG_VERSION");
  let description = if cfg!(debug_assertions) {
    "settings.general.version.development"
  } else {
    "settings.general.version.release"
  };
  let value = Text::new(&ctx.t_args("settings.general.version.value", [("version", version)]))
    .id("settings-version-value")
    .variant(ExtraTypography::MonoKey.role())
    .color(PaletteColor::TextSecondary)
    .nowrap();
  side_row(
    Label::new("settings-version", ctx.t("settings.general.version.title"))
      .description(ctx.t_args(description, [("version", version)])),
    value,
    Place::Within,
    false,
  )
}

/// A place's row: its element id and the keys of what it says.
struct PlaceKeys {
  id: &'static str,
  title: &'static str,
  description: &'static str,
  unknown: &'static str,
}

/// A file or folder of this computer under its title and what it holds, with Copy; `None` while it is read, when the
/// row holds only its words, and `Some(None)` when it is unknown, which the row says.
fn place(ctx: &mut Ctx, keys: &PlaceKeys, path: Option<Option<&PathBuf>>) -> Element {
  let label = Label::new(keys.id, ctx.t(keys.title)).description(ctx.t(keys.description));
  let control: Element = match path {
    Some(Some(path)) => {
      let shown = path.display().to_string();
      let copied = shown.clone();
      CommandRow::new(format!("{}-path", keys.id), shown, ctx.t("common.copy"), move || {
        copy_to_clipboard(&copied)
      })
      .one_line()
      .render(ctx)
    }
    Some(None) => field_error(&format!("{}-unknown", keys.id), &ctx.t(keys.unknown)).into(),
    None => Element::new(),
  };
  stacked_row(label, control, Place::Within)
}
