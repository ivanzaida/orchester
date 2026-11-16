//! A choice of a few options as segments (*Kind*, *Method*, *Authorization* and the *Access* rows of A32 `gLLEv`): the
//! chosen one is held as its value's text, which the segment's id ends with.

use std::sync::Arc;

use lurq::{core::Signal, node::Element};

use crate::ui::{
  pressable::Focus,
  tabs::{Segment, SegmentFace, segmented},
};

/// The segments of `options` (a value and its words) under the element id `id`; choosing one sets `value` to it, unless
/// the form is `locked`.
pub(super) fn segments(
  id: &str,
  value: &Signal<String>,
  options: impl IntoIterator<Item = (&'static str, Arc<str>)>,
  locked: bool,
) -> Element {
  let chosen = value.get();
  let segments = options.into_iter().map(|(option, words)| {
    let value = value.clone();
    Segment::new(
      format!("{id}-{option}"),
      SegmentFace::Text(words),
      chosen == option,
      move || {
        if !locked {
          value.set(option.to_owned());
        }
      },
    )
  });
  segmented(id, segments, Focus::Keyboard).into()
}

/// The value of `options` whose text `value` holds, else the first's.
pub(super) fn chosen<T: Copy>(value: &Signal<String>, options: &[(&'static str, T)]) -> T {
  let held = value.get();
  options.iter().find(|(text, _)| *text == held).unwrap_or(&options[0]).1
}

/// The text `options` holds `value` by, else the first's.
pub(super) fn text_of<T: Copy + PartialEq>(value: T, options: &[(&'static str, T)]) -> &'static str {
  options
    .iter()
    .find(|(_, option)| *option == value)
    .unwrap_or(&options[0])
    .0
}
