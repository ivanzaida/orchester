//! *Plan limits* and its *Warning threshold* (`Warning threshold` `t5qdp` in `01 · General`): the share of a plan
//! window at which every project's Usage counts its limits as nearly used, a whole percentage from 1 to 100, kept for
//! Orchester rather than a project because limits belong to accounts. This is its one place: Usage › Plan limits' menu
//! opens it here, the group scrolled into view. It is read through its query, which the window read as the session
//! started; a value that is a percentage saves as it is typed, off the UI thread, one save at a time and the newest
//! last, and a change from elsewhere fills the field while none runs. A save that failed keeps what was typed and says
//! why under the row.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::usage::limits::WarningThreshold;
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use crate::{
  queries::{self, Session, app},
  screens::settings::{save_state::SaveState, serial::Serial},
  services::Services,
  ui::{
    field::{FieldInput, field_error},
    message::message_text,
    setting_rows::{SettingGroup, SettingRow},
    skeleton::{self, Skeleton},
  },
};

const GROUP: &str = "settings-plan-limits";
const ID: &str = "settings-threshold";
/// The value's field and its unit (`Box` `C8WIQw` in `01 · General`).
const FIELD_WIDTH: f32 = 120.0;
/// The value's bone while it is read: a two-digit percentage.
const VALUE_BONE: f32 = 14.0;

/// Whether the route brings *Plan limits* into view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InView(pub bool);

impl DevtoolsInspectable for InView {}

/// *Plan limits* with its threshold, scrolled into view when a route newly asks for it.
pub struct PlanLimitsGroup {
  group: ElementRef,
  /// What the route last asked, so each opening scrolls once.
  followed: Mutex<bool>,
}

impl Component for PlanLimitsGroup {
  type Props = InView;

  fn create(_: &mut Ctx) -> Self {
    Self {
      group: ElementRef::new(),
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let InView(asked) = *ctx.props::<InView>();
    let followed = std::mem::replace(&mut *self.followed.lock(), asked);
    if asked && !followed {
      ctx.scroll_into_view(&self.group);
    }
    let group =
      SettingGroup::new(GROUP, ctx.t("settings.general.planLimits")).row(ctx.mount_keyed::<ThresholdRow>(ID, ()));
    Column::new()
      .ref_element(self.group.clone())
      .width(Dimension::Pct(100.0))
      .child(group)
  }
}

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
  /// The stored threshold the field last took, so a change from elsewhere fills it again.
  seen: Mutex<Option<WarningThreshold>>,
  saves: Saves,
}

impl Component for ThresholdRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    // The workspace counts the change once saved, which reads the threshold again (`queries`).
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
      seen: Mutex::new(None),
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
    let session = Session::of(ctx);
    let read = queries::state(&ctx.query(app::usage_threshold(session)));
    self.take(read.data);
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
  /// Fills the field with the stored threshold when it is first read, and again when it changed elsewhere while no save
  /// of the row's runs; a threshold the row asked for itself is in the field already.
  fn take(&self, stored: Option<WarningThreshold>) {
    let Some(threshold) = stored else {
      return;
    };
    if self.seen.lock().replace(threshold) == Some(threshold) {
      return;
    }
    let mut known = self.known.lock();
    let first = *known == Known::Unread;
    if *known == Known::Asked(threshold) || (!first && self.saves.state().get_untracked().is_pending()) {
      return;
    }
    *known = Known::Asked(threshold);
    drop(known);
    self.invalid.set(false);
    self.text.set(threshold.percent().to_string());
  }
}
