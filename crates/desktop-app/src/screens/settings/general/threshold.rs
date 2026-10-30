//! *Warning threshold* (`Warning threshold` `t5qdp` in `01 · General`): the share of a plan window at which every
//! project's Usage counts its limits as nearly used, a whole percentage from 1 to 100, kept for Orchester rather than a
//! project because limits belong to accounts; the same setting Usage › Plan limits' menu changes. It is read when the
//! row shows, and a value that is a percentage saves as it is typed, off the UI thread, one save at a time and the
//! newest last.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::limits::WarningThreshold;
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use crate::{
  screens::settings::{save_state::SaveState, serial::Serial},
  services::Services,
  ui::{
    field::{FieldInput, field_error},
    message::message_text,
    setting_rows::SettingRow,
    skeleton::{self, Skeleton},
  },
};

const ID: &str = "settings-threshold";
/// The value's field and its unit (`Box` `C8WIQw` in `01 · General`).
const FIELD_WIDTH: f32 = 120.0;
/// The value's bone while it is read: a two-digit percentage.
const VALUE_BONE: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read(WarningThreshold);

impl DevtoolsInspectable for Read {}

type Saves = Serial<WarningThreshold, (), Message>;

/// What the row knows of the stored threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Known {
  /// Not read yet: nothing typed is saved.
  Unread,
  /// The threshold as read, or the newest one asked to be saved.
  Asked(WarningThreshold),
  /// A save failed: the next percentage typed is saved, even the one that failed.
  Unsure,
}

pub struct ThresholdRow {
  text: Signal<String>,
  /// A typed value that is not a whole percentage.
  invalid: Signal<bool>,
  known: Arc<Mutex<Known>>,
  saves: Saves,
}

impl Component for ThresholdRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    let action: FutureAction<WarningThreshold, (), Message> = ctx.future_action(move |threshold: WarningThreshold| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.set_usage_warning_threshold(threshold))
          .await?
          .map_err(|error| {
            tracing::error!(%error, "cannot save the usage warning threshold");
            messages::store_error(&error)
          })
      }
    });
    let saves = Serial::new(ctx, action);
    let row = Self {
      text: ctx.signal(String::new()),
      invalid: ctx.signal(false),
      known: Arc::new(Mutex::new(Known::Unread)),
      saves: saves.clone(),
    };
    // Saves on change, whatever changed the field's text; a value already asked for is not saved again.
    let (invalid, known) = (row.invalid.clone(), Arc::clone(&row.known));
    ctx.watch(&row.text, move |typed: &String| {
      let mut known = known.lock();
      if *known == Known::Unread {
        return;
      }
      match WarningThreshold::parse(typed) {
        Ok(threshold) => {
          invalid.set(false);
          if *known != Known::Asked(threshold) {
            *known = Known::Asked(threshold);
            saves.save(threshold);
          }
        }
        Err(_) => invalid.set(true),
      }
    });
    let known = Arc::clone(&row.known);
    ctx.watch(&row.saves.state(), move |state: &FutureState<(), Message>| {
      if state.is_rejected() {
        *known.lock() = Known::Unsure;
      }
    });
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let read = self.stored(ctx);
    if let Some(Read(threshold)) = read.data {
      let mut known = self.known.lock();
      if *known == Known::Unread {
        *known = Known::Asked(threshold);
        drop(known);
        self.text.set(threshold.percent().to_string());
      }
    }
    let state = self.saves.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, self.saves.outcome(&state));
    }
    let field = FieldInput::new(
      format!("{ID}-value"),
      ctx.t("settings.general.threshold.title"),
      self.text.clone(),
    )
    .unit(ctx.t("settings.general.threshold.unit"))
    .mono()
    .width(Dimension::Px(FIELD_WIDTH));
    let field = match (read.data.is_some(), self.invalid.get()) {
      (false, _) => field.locked(),
      (true, true) => field.invalid(),
      (true, false) => field,
    };
    let problem = if self.invalid.get() {
      Some(ctx.t("settings.general.threshold.invalid").to_string())
    } else if let Some(reason) = read
      .error
      .as_ref()
      .or(state.error.as_ref().filter(|_| !self.saves.busy(&state)))
    {
      let key = if read.error.is_some() {
        "settings.general.threshold.unreadable"
      } else {
        "settings.general.threshold.notSaved"
      };
      let reason = message_text(ctx, reason);
      Some(ctx.t_args(key, [("reason", &*reason)]).to_string())
    } else {
      None
    };
    // While the threshold is read, its value is a bone in its field (`13 · App settings · General · loading` `JVv11`).
    let field = if read.is_pending() && read.data.is_none() {
      let bone = skeleton::unit_field(ctx, FIELD_WIDTH, VALUE_BONE, &ctx.t("settings.general.threshold.unit"));
      Skeleton::new(format!("{ID}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, bone)
    } else {
      field.render(ctx)
    };
    SettingRow::new(ID, ctx.t("settings.general.threshold.title"))
      .hint(ctx.t("settings.general.threshold.hint"))
      .info(ctx.t("settings.general.threshold.description"))
      .control(field)
      .below_all(problem.map(|problem| field_error(&format!("{ID}-error"), &problem).into()))
      .render(ctx)
  }
}

impl ThresholdRow {
  /// The threshold as stored, read when the row shows.
  fn stored(&self, ctx: &mut Ctx) -> lurq::app::ctx::FutureState<Read, Message> {
    let services = Services::of(ctx);
    let (workspace, hold) = (services.workspace, services.hold);
    ctx
      .future((), move |()| {
        let (workspace, hold) = (workspace.clone(), hold.clone());
        async move {
          hold.released().await;
          background::run(move || workspace.usage_warning_threshold())
            .await?
            .map(Read)
            .map_err(|error| {
              tracing::error!(%error, "the usage warning threshold cannot be read");
              messages::store_error(&error)
            })
        }
      })
      .state()
      .get()
  }
}
