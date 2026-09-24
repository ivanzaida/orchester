//! Keyboard behavior of a form, from lurq's form feature: Tab and Shift+Tab move through the
//! form's fields and available buttons in the order they appear (wrapping at either end, so
//! focus stays in a dialog), Enter in a single-line field submits, and Enter or Space
//! presses the focused button. The values stay in the screen's own signals and are checked
//! by the rules in core, so the lurq form only carries the submit.

use std::sync::Arc;

use lurq::{
  components::{Form, FormProps},
  node::Element,
};

/// What submitting does. `None` while the form cannot be submitted (it is incomplete, or its
/// work is already running): Enter in a field then does nothing, as its submit button does.
pub type Submit = Option<Arc<dyn Fn() + Send + Sync>>;

pub fn form(submit: Submit, content: impl Into<Element>) -> Element {
  // lurq only moves focus inside a node that has a submit handler, so the form always has one.
  let props = FormProps::default().on_submit_data(move |_| {
    if let Some(submit) = &submit {
      submit();
    }
  });
  Form::element(props, content)
}
