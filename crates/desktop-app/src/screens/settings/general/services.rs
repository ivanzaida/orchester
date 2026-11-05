//! *Services* and its *Services at once*: how many services the runs of every project may run together on this
//! computer, a whole number from 1 to 32 (8 until changed), kept for Orchester rather than a project; each project's
//! share in its Runs › Limits counts within it (`docs/architecture/agent-testing.md`, decision 3). It has no frame of
//! its own in the design, so it is built as its neighbour *Warning threshold* (`threshold`): read through its query,
//! a whole number saves as it is typed, off the UI thread, one save at a time and the newest last, and a change from
//! elsewhere (`update_project_limits`' `computer_service_slots`) fills the field while none runs. A save that failed
//! keeps what was typed and says why under the row.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::limits::ComputerServiceSlots;
use orchester_workspace::{
  background,
  budget::read_computer_service_slots,
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

const GROUP: &str = "settings-services";
const ID: &str = "settings-service-slots";
/// The value's field and its unit, as wide as *Warning threshold*'s (`Box` `C8WIQw` in `01 · General`).
const FIELD_WIDTH: f32 = 120.0;
/// The value's bone while it is read: a two-digit count.
const VALUE_BONE: f32 = 14.0;

/// *Services* with the computer's *Services at once*.
pub fn group(ctx: &mut Ctx) -> SettingGroup {
  SettingGroup::new(GROUP, ctx.t("settings.general.services.title")).row(ctx.mount_keyed::<ServiceSlotsRow>(ID, ()))
}

type Saves = Serial<ComputerServiceSlots, (), Message>;

/// What the row knows of the stored limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Known {
  /// Not read yet: nothing typed is saved.
  Unread,
  /// The limit as read, or the newest one asked to be saved.
  Asked(ComputerServiceSlots),
  /// A save failed: the next count typed is saved, even the one that failed.
  Unsure,
}

pub struct ServiceSlotsRow {
  text: Signal<String>,
  /// A typed value that is not a whole number from 1 to 32.
  invalid: Signal<bool>,
  known: Arc<Mutex<Known>>,
  /// The stored limit the field last took, so a change from elsewhere fills it again.
  seen: Mutex<Option<ComputerServiceSlots>>,
  saves: Saves,
}

impl Component for ServiceSlotsRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    // The workspace counts the change once saved, which reads the limit again here and in every Runs › Limits.
    let action: FutureAction<ComputerServiceSlots, (), Message> =
      ctx.future_action(move |slots: ComputerServiceSlots| {
        let workspace = workspace.clone();
        async move {
          background::run(move || workspace.set_service_slots(slots))
            .await?
            .map_err(|error| {
              tracing::error!(%error, "cannot save the computer's services at once");
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
      match read_computer_service_slots(typed) {
        Ok(slots) => {
          invalid.set(false);
          if *known != Known::Asked(slots) {
            *known = Known::Asked(slots);
            saves.save(slots);
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
    let read = queries::state(&ctx.query(app::service_slots(session)));
    self.take(read.data);
    let state = self.saves.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, self.saves.outcome(&state));
    }
    let unit = ctx.t("settings.general.services.unit");
    let field = FieldInput::new(
      format!("{ID}-value"),
      ctx.t("settings.general.services.label"),
      self.text.clone(),
    )
    .unit(unit.clone())
    .mono()
    .width(Dimension::Px(FIELD_WIDTH));
    let field = match (read.data.is_some(), self.invalid.get()) {
      (false, _) => field.locked(),
      (true, true) => field.invalid(),
      (true, false) => field,
    };
    let problem = if self.invalid.get() {
      Some(ctx.t("settings.general.services.invalid").to_string())
    } else if let Some(reason) = read
      .error
      .as_ref()
      .or(state.error.as_ref().filter(|_| !self.saves.busy(&state)))
    {
      let key = if read.error.is_some() {
        "settings.general.services.unreadable"
      } else {
        "settings.general.services.notSaved"
      };
      let reason = message_text(ctx, reason);
      Some(ctx.t_args(key, [("reason", &*reason)]).to_string())
    } else {
      None
    };
    // While the limit is read, its value is a bone in its field, as the threshold's is.
    let field = if read.is_pending() && read.data.is_none() {
      let bone = skeleton::unit_field(ctx, FIELD_WIDTH, VALUE_BONE, &unit);
      Skeleton::new(format!("{ID}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, bone)
    } else {
      field.render(ctx)
    };
    SettingRow::new(ID, ctx.t("settings.general.services.label"))
      .hint(ctx.t("settings.general.services.hint"))
      .info(ctx.t("settings.general.services.description"))
      .control(field)
      .below_all(problem.map(|problem| field_error(&format!("{ID}-error"), &problem).into()))
      .render(ctx)
  }
}

impl ServiceSlotsRow {
  /// Fills the field with the stored limit when it is first read, and again when it changed elsewhere while no save of
  /// the row's runs; a limit the row asked for itself is in the field already.
  fn take(&self, stored: Option<ComputerServiceSlots>) {
    let Some(slots) = stored else {
      return;
    };
    if self.seen.lock().replace(slots) == Some(slots) {
      return;
    }
    let mut known = self.known.lock();
    let first = *known == Known::Unread;
    if *known == Known::Asked(slots) || (!first && self.saves.state().get_untracked().is_pending()) {
      return;
    }
    *known = Known::Asked(slots);
    drop(known);
    self.invalid.set(false);
    self.text.set(slots.get().to_string());
  }
}
