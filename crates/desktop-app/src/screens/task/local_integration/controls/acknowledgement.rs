use orchester_core::integration::local::{Checkout, LocalOperation};

use super::{Mode, Props};
use crate::screens::task::local_integration::Selection;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ReviewIdentity {
  mode: Mode,
  main: Checkout,
  checked_current: bool,
  operation: Option<LocalOperation>,
  selection: Selection,
  message: String,
  preservation_name: String,
}

impl ReviewIdentity {
  pub(super) fn of(props: &Props, mode: &Mode) -> Self {
    Self {
      mode: mode.clone(),
      main: props.view.checkout.clone(),
      checked_current: props.view.checked_current,
      operation: props.view.operation.clone(),
      selection: props.selected.get(),
      message: props.message.get(),
      preservation_name: props.name.get(),
    }
  }
}

#[derive(Clone)]
pub(in crate::screens::task::local_integration) struct AcknowledgementState {
  pub agree: lurq::core::Signal<bool>,
  /// Window automation tried to tick it: the refusal shows until the person ticks it or the decision changes.
  pub refused: lurq::core::Signal<bool>,
  pub(super) identity: std::sync::Arc<parking_lot::Mutex<Option<ReviewIdentity>>>,
}
impl AcknowledgementState {
  pub fn new(ctx: &mut lurq::app::ctx::Ctx) -> Self {
    Self {
      agree: ctx.signal(false),
      refused: ctx.signal(false),
      identity: std::sync::Arc::new(parking_lot::Mutex::new(None)),
    }
  }
}
