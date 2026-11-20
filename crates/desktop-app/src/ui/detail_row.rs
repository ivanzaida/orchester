//! Detail row (`Detail row` `paUqV`): lurq's `ui::DataList` of one item in the kit's recipe
//! (`theme::kit::detail_row`): what a value is in 12 `text_muted` in a 108 column, and the value 12 after it in
//! 12/1.45 `text_primary`, wrapping; 5 over and under them. An identifier's value is in Geist Mono (`Branch` `YHJhy`).

use lurq::{layout::Alignment, node::Element, ui::DataList};

use crate::theme::kit::detail_row::MONO;

/// How a detail's value is set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DetailValue {
  #[default]
  Text,
  /// An identifier: a branch, a hash, a path.
  Mono,
}

/// `key` and its `value`; the row's element id is `id`, the key's `<id>-term` and the value's `<id>-value`.
pub fn detail_row(id: &str, key: &str, value: &str, kind: DetailValue) -> DataList {
  list(id).state(MONO, kind == DetailValue::Mono).text(key, value)
}

/// `key` beside a value of more than one line or style, e.g. a lead line over the value (`Detail ·
/// Permissions` `Up8y8`); the key's element id is `<id>-term`, and `value` takes the rest of the row.
pub fn detail_row_of(id: &str, key: &str, value: impl Into<Element>) -> DataList {
  list(id).item(key, value)
}

/// The key at the top of a value that wraps.
fn list(id: &str) -> DataList {
  DataList::new().id(id.to_owned()).align(Alignment::Start)
}
