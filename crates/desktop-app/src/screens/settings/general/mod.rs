//! Settings › General (`01 · General` `l838Ty` in the new `orchester.pen`): *Appearance* with the window's theme and
//! the honest note under it (`theme`), *Plan limits* with the warning threshold every project's Usage draws
//! (`threshold`), and *This computer*: the app's version and whether it is a release or a development build, and where
//! its application database and its log are, each with Copy. What a row is for is in its (i) tooltip. A change saves
//! as it is made, off the UI thread, and the header's save state follows it (`super::save_state`). Nothing here widens
//! what agents may do, so no control is a person's alone.

mod theme;
mod threshold;

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  clipboard::copy_to_clipboard,
  node::Element,
};
use orchester_workspace::{app_paths, background};

use super::frame;
use crate::{
  services::Services,
  theme::ExtraTypography,
  ui::{
    copied_value::CopiedValue,
    setting_rows::{HintTone, SettingGroup, SettingRow, value},
    skeleton::{self, Skeleton},
  },
};

/// A path's bone while it is read (`Application data` in `JVv11`).
const PATH_BONE: f32 = 162.0;

/// The tab, scrolling under the header.
pub fn tab(ctx: &mut Ctx) -> Element {
  let pane = frame::pane("settings-general")
    .child(
      SettingGroup::new("settings-appearance", ctx.t("settings.general.appearance"))
        .row(ctx.mount_keyed::<theme::ThemeRow>("settings-theme", ()))
        .note(ctx.t(theme::NOTE)),
    )
    .child(
      SettingGroup::new("settings-plan-limits", ctx.t("settings.general.planLimits"))
        .row(ctx.mount_keyed::<threshold::ThresholdRow>("settings-threshold", ())),
    )
    .child(ctx.mount_keyed::<ThisComputer>("settings-computer", ()));
  frame::column("settings-general-scroll", pane)
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
    let services = Services::of(ctx);
    let (workspace, hold) = (services.workspace, services.hold);
    let places = ctx
      .future((), move |()| {
        let (workspace, hold) = (workspace.clone(), hold.clone());
        async move {
          hold.released().await;
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
    let rows = [
      version(ctx),
      place(ctx, &data, places.as_ref().map(|places| places.database.as_ref())),
      place(ctx, &log_keys, places.as_ref().map(|places| places.log.as_ref())),
    ];
    SettingGroup::new("settings-computer", ctx.t("settings.general.computer")).rows(rows)
  }
}

/// *Version*: the version and whether this build is a release or a development one, as plain text at the row's end.
fn version(ctx: &mut Ctx) -> Element {
  let version = env!("CARGO_PKG_VERSION");
  let (value_key, description) = if cfg!(debug_assertions) {
    (
      "settings.general.version.developmentValue",
      "settings.general.version.development",
    )
  } else {
    (
      "settings.general.version.releaseValue",
      "settings.general.version.release",
    )
  };
  let shown = ctx.t_args(value_key, [("version", version)]);
  SettingRow::new("settings-version", ctx.t("settings.general.version.title"))
    .info(ctx.t_args(description, [("version", version)]))
    .control(value("settings-version-value", &shown))
    .render(ctx)
}

/// A place's row: its element id and the keys of what it says.
struct PlaceKeys {
  id: &'static str,
  title: &'static str,
  description: &'static str,
  unknown: &'static str,
}

/// A file or folder of this computer with Copy, what it holds in the row's tooltip; `None` while it is read, when its
/// path is a bone and Copy waits for it (`13 · App settings · General · loading` `JVv11`), and `Some(None)` when it is
/// unknown, which the row says.
fn place(ctx: &mut Ctx, keys: &PlaceKeys, path: Option<Option<&PathBuf>>) -> Element {
  let row = SettingRow::new(keys.id, ctx.t(keys.title)).info(ctx.t(keys.description));
  let row = match path {
    Some(Some(path)) => {
      let shown = path.display().to_string();
      let copied = shown.clone();
      let copy = CopiedValue::new(format!("{}-path", keys.id), shown, ctx.t("common.copy"), move || {
        let done = copy_to_clipboard(&copied);
        if !done {
          tracing::warn!("a path of this computer could not be put on the clipboard");
        }
        done
      })
      .render(ctx);
      row.control(copy)
    }
    Some(None) => row.toned_hint(ctx.t(keys.unknown), HintTone::Danger),
    None => {
      let bone = skeleton::line(ctx, ExtraTypography::PropertyMono.role(), PATH_BONE);
      let bone = Skeleton::new(format!("{}-reading", keys.id), ctx.t("settings.reading"))
        .inline()
        .show(ctx, bone);
      row.control(bone)
    }
  };
  row.render(ctx)
}
