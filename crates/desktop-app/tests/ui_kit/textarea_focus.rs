//! Pointer focus covers the text area's empty body and padding, not only its first line.

use lurq::{
  app::events::MouseButton,
  components::{Column, TextInput},
  core::Signal,
  layout::quad::QuadContent,
  node::{dimension::Dimension, padding::Padding},
};
use orchester_app::ui::field::FieldInput;

use crate::harness::{Kit, assert_close};

fn area(value: &Signal<String>, compact: bool, locked: bool) -> Kit {
  area_at_width(value, compact, locked, 400.0)
}

fn area_at_width(value: &Signal<String>, compact: bool, locked: bool, width: f32) -> Kit {
  let shown = value.clone();
  Kit::show(move |ctx| {
    let field =
      FieldInput::new("note", "Note for Implement", shown.clone()).placeholder("What should Implement change?");
    let field = if compact {
      field.compact_text_area()
    } else {
      field.text_area()
    };
    let field = if locked { field.locked() } else { field };
    Column::new()
      .width(Dimension::Px(width))
      .child(field.render(ctx))
      .child(FieldInput::new("elsewhere", "Other field", Signal::new(String::new())).render(ctx))
      .into()
  })
}

fn click_at(kit: &mut Kit, x: f32, y: f32) {
  kit.tree.mouse_move(x, y);
  kit.tree.mouse_down(x, y, MouseButton::Left);
  kit.tree.mouse_up(x, y, MouseButton::Left);
  kit.pass();
}

#[test]
fn textarea_focus_lurq_input_with_its_own_size_and_padding_covers_the_whole_box() {
  let shown = Signal::new(String::new());
  let mut kit = Kit::show(move |_ctx| {
    Column::new()
      .child(
        TextInput::new(shown.clone())
          .id("raw-note")
          .placeholder("What should Implement change?")
          .multiline()
          .rows(1, 12)
          .width(Dimension::Px(400.0))
          .min_height(Dimension::Px(84.0))
          .padding(Padding::symmetric(10.0, 7.0)),
      )
      .into()
  });
  let frame = kit.bounds("raw-note");
  for (horizontal, vertical) in [(0.5, 0.7), (0.99, 0.2), (0.01, 0.5), (0.5, 0.03), (0.5, 0.97)] {
    click_at(&mut kit, 600.0, 300.0);
    assert!(kit.tree.focused_element().is_none());
    click_at(
      &mut kit,
      frame.x + frame.width * horizontal,
      frame.y + frame.height * vertical,
    );
    assert_eq!(kit.tree.focused_element().and_then(|node| node.id()), Some("raw-note"));
  }
}

#[test]
fn textarea_focus_includes_empty_body_right_edge_and_padding() {
  let mut missed = Vec::new();
  for compact in [false, true] {
    for initial in ["", "Existing note"] {
      for (region, horizontal, vertical) in [
        ("first line", 0.1, 0.2),
        ("blank body", 0.5, 0.7),
        ("right of text", 0.9, 0.2),
        ("left padding", 0.01, 0.5),
        ("right padding", 0.99, 0.5),
        ("top padding", 0.5, 0.03),
        ("bottom padding", 0.5, 0.97),
      ] {
        let value = Signal::new(initial.to_owned());
        let mut kit = area(&value, compact, false);
        let frame = kit.bounds("note-box");
        kit.click("elsewhere");
        click_at(
          &mut kit,
          frame.x + frame.width * horizontal,
          frame.y + frame.height * vertical,
        );
        if kit.tree.focused_element().and_then(|node| node.id()) != Some("note") {
          missed.push(format!("{region}, compact={compact}, value={initial:?}"));
          continue;
        }
        kit
          .tree
          .key_down("x".to_owned(), "KeyX".to_owned(), false, false, false);
        kit.pass();
        assert_eq!(value.get().len(), initial.len() + 1, "typing after {region}");
      }
    }
  }
  assert!(missed.is_empty(), "clicks did not focus the textarea: {missed:?}");
}

#[test]
fn textarea_focus_locked_area_never_takes_pointer_focus_or_typing() {
  for compact in [false, true] {
    for initial in ["", "Existing note"] {
      let value = Signal::new(initial.to_owned());
      let mut kit = area(&value, compact, true);
      let frame = kit.bounds("note-box");
      for (horizontal, vertical) in [(0.1, 0.2), (0.5, 0.7), (0.01, 0.5), (0.99, 0.5), (0.5, 0.97)] {
        kit.click("elsewhere");
        click_at(
          &mut kit,
          frame.x + frame.width * horizontal,
          frame.y + frame.height * vertical,
        );
        assert_ne!(kit.tree.focused_element().and_then(|node| node.id()), Some("note"));
        kit
          .tree
          .key_down("x".to_owned(), "KeyX".to_owned(), false, false, false);
        kit.pass();
        assert_eq!(value.get(), initial);
      }
    }
  }
}

#[test]
fn textarea_focus_preserves_text_insets_growth_and_scroll_after_twelve_lines() {
  let value = Signal::new("Existing note".to_owned());
  let mut kit = area(&value, false, false);
  let frame = kit.bounds("note-box");
  let text = kit
    .tree
    .painted_quads()
    .into_iter()
    .find(|quad| matches!(&quad.content, QuadContent::Text { text, .. } if text == "Existing note"))
    .expect("the textarea paints its value");
  assert_close(frame.height, 84.0, "minimum height");
  assert_close(text.x - frame.x, 10.0, "text inset at the side");
  assert_close(text.y - frame.y, 7.0, "text inset at the top");
  value.set("line\n".repeat(7));
  kit.pass();
  assert!(
    kit.bounds("note-box").height > frame.height,
    "more lines grow the field"
  );
  value.set("line\n".repeat(11));
  kit.pass();
  let capped = kit.bounds("note-box");
  value.set("line\n".repeat(19));
  kit.pass();
  assert_close(kit.bounds("note-box").height, capped.height, "twelve-line cap");
  kit.click("note");
  kit
    .tree
    .key_down("End".to_owned(), "End".to_owned(), false, true, false);
  kit.pass();
  let text = kit
    .tree
    .painted_quads()
    .into_iter()
    .find(|quad| matches!(&quad.content, QuadContent::Text { text, .. } if text.contains("line\n")))
    .expect("the overflowing textarea paints its value");
  assert!(text.y < capped.y + 7.0, "the caret scrolls the text to the last line");
  assert_close(text.clip.y - capped.y, 7.0, "scroll clip starts below the top inset");
  assert_close(
    text.clip.height,
    capped.height - 14.0,
    "scroll clip keeps both vertical insets",
  );
}

#[test]
fn textarea_focus_soft_wrapping_keeps_insets_and_caps_both_area_sizes() {
  for compact in [false, true] {
    for width in [400.0, 160.0] {
      let value = Signal::new(String::new());
      let mut kit = area_at_width(&value, compact, false, width);
      let empty = kit.bounds("note-box");
      assert_close(empty.height, if compact { 50.0 } else { 84.0 }, "minimum height");
      let paragraph = "Words that wrap naturally at the field's right inset. ";
      value.set(paragraph.repeat(6));
      kit.pass();
      assert!(
        kit.bounds("note-box").height > empty.height,
        "soft-wrapped rows grow the area"
      );
      value.set(paragraph.repeat(100));
      kit.pass();
      let capped = kit.bounds("note-box");
      value.set(paragraph.repeat(200));
      kit.pass();
      assert_close(kit.bounds("note-box").height, capped.height, "soft-wrapped height cap");
      let text = kit
        .tree
        .painted_quads()
        .into_iter()
        .find(|quad| matches!(&quad.content, QuadContent::Text { text, .. } if text.starts_with(paragraph)))
        .expect("the wrapped textarea paints its value");
      assert_close(text.clip.x - capped.x, 10.0, "wrapped text's left inset");
      assert_close(text.clip.width, width - 20.0, "wrap width excludes both side insets");
      assert_close(text.clip.y - capped.y, 7.0, "wrapped text's top inset");
    }
  }
}
