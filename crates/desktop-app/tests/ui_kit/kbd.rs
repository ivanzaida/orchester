//! The key cap (`wMPlq`) is lurq's `ui::Kbd` in the design's look: the keys as written, one 18 high cap.

use orchester_app::ui::kbd::key_cap;

use crate::harness::{Kit, assert_close};

#[test]
fn kbd_key_cap_writes_its_keys_in_one_cap_18_high() {
  let mut kit = Kit::show(|_| key_cap("cap", "⌘K"));
  assert_eq!(kit.texts("cap"), ["⌘K"]);
  assert_close(kit.bounds("cap").height, 18.0, "the cap's height");
}
