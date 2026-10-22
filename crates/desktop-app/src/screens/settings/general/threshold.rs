//! *Warning threshold* (`Row · Warning threshold` `SPJjc` in `hUaXB`): the share of a plan window at which every
//! project's Usage counts its limits as nearly used, a whole percentage from 1 to 100, kept for Orchester rather than a
//! project because limits belong to accounts. The frame draws it as planned; the person chose a working field here
//! (2026-10-22), the same setting Usage › Plan limits' menu changes. It is read when the row shows, and a value that is
//! a percentage saves as it is typed, off the UI thread.

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
use orchester_core::usage::limits::WarningThreshold;
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use crate::{
  screens::settings::save_state::{Outcome, SaveState},
  services::Services,
  ui::{
    field::{FieldInput, field_error},
    form_section::{Label, Place, side_row},
    message::message_text,
  },
};

const ID: &str = "settings-threshold";
/// The value's field: three digits and the unit (`Warning threshold` in `p732q`, as Usage draws it).
const FIELD_WIDTH: f32 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Read(WarningThreshold);

impl DevtoolsInspectable for Read {}

type SaveAction = FutureAction<WarningThreshold, (), Message>;

pub struct ThresholdRow {
  text: Signal<String>,
  /// A typed value that is not a whole percentage.
  invalid: Signal<bool>,
  /// The threshold as last read or saved; `None` until it is read, when nothing typed is saved.
  last: Arc<Mutex<Option<WarningThreshold>>>,
  save: SaveAction,
}

impl Component for ThresholdRow {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let workspace = Services::of(ctx).workspace;
    let save: SaveAction = ctx.future_action(move |threshold: WarningThreshold| {
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
    let row = Self {
      text: ctx.signal(String::new()),
      invalid: ctx.signal(false),
      last: Arc::default(),
      save: save.clone(),
    };
    // Saves on change, whatever changed the field's text; an unchanged value is not saved again.
    let (invalid, last) = (row.invalid.clone(), Arc::clone(&row.last));
    ctx.watch(&row.text, move |typed: &String| {
      let mut last = last.lock();
      let Some(saved) = *last else {
        return;
      };
      match WarningThreshold::parse(typed) {
        Ok(threshold) => {
          invalid.set(false);
          if saved != threshold {
            *last = Some(threshold);
            save.run(threshold);
          }
        }
        Err(_) => invalid.set(true),
      }
    });
    row
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let read = self.stored(ctx);
    if let Some(Read(threshold)) = read.data {
      let mut last = self.last.lock();
      if last.is_none() {
        *last = Some(threshold);
        drop(last);
        self.text.set(threshold.percent().to_string());
      }
    }
    let state = self.save.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(ID, Outcome::of(&state));
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
    let row = side_row(
      Label::new(ID, ctx.t("settings.general.threshold.title")).description(ctx.t("settings.general.threshold.description")),
      field.render(ctx),
      Place::Within,
      false,
    );
    let problem = if self.invalid.get() {
      Some(ctx.t("settings.general.threshold.invalid").to_string())
    } else if let Some(reason) = read.error.as_ref().or(state.error.as_ref().filter(|_| !state.is_pending())) {
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
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(row)
      .with_children(problem.map(|problem| {
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding_bottom(SpacingSize::Sm)
          .child(field_error(&format!("{ID}-error"), &problem))
      }))
  }
}

impl ThresholdRow {
  /// The threshold as stored, read when the row shows.
  fn stored(&self, ctx: &mut Ctx) -> lurq::app::ctx::FutureState<Read, Message> {
    let workspace = Services::of(ctx).workspace;
    ctx
      .future((), move |()| {
        let workspace = workspace.clone();
        async move {
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
