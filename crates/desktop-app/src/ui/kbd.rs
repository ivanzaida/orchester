//! Key cap (`Kbd` `wMPlq`): lurq's `ui::Kbd` with the keys as one cap, in the theme's recipe (`theme::kit`).

use lurq::{components::Row, node::Element, ui::Kbd};

/// A cap with `keys`, written as they are; `id` names it.
pub fn key_cap(id: &str, keys: &str) -> Element {
  // A cap keeps its width beside words that give way.
  Row::new()
    .flex_shrink(0.0)
    .child(Kbd::new(keys).id(id.to_owned()))
    .into()
}
