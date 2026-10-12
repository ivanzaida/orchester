use orchester_core::integration::local::{Checkout, LocalOperation, LocalPath};

use super::{Mode, Props};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ReviewIdentity {
  mode: Mode,
  main: Checkout,
  checked_current: bool,
  operation: Option<LocalOperation>,
  selection: Vec<LocalPath>,
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
      selection: props.selected.get().0,
      message: props.message.get(),
      preservation_name: props.name.get(),
    }
  }
}
