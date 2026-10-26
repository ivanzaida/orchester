//! The Summary's brief and criteria read as Markdown, as Details shows them.

use lurq::layout::quad::QuadContent;

use crate::Page;

#[test]
fn summary_brief_and_criteria_read_as_markdown() {
  let mut page = Page::seeded("task-states");
  page.open("running");
  let brief = page.window.texts("task-brief-text").expect("the brief");
  let criteria = page.window.texts("task-criteria").expect("the criteria");
  let shown = [brief.join(" "), criteria.join(" ")].join(" ");
  for markup in ["**", "`", "]("] {
    assert!(!shown.contains(markup), "no raw Markdown: {shown}");
  }
  // A paragraph per list item, each after its marker, and the link pressable on its own.
  assert_eq!(brief.iter().filter(|text| *text == "-").count(), 2, "{brief:?}");
  assert!(brief.iter().any(|text| text == "retry guide"), "{brief:?}");
  assert!(page.text("task-criterion-1").starts_with("1 Deliveries that fail"));
  // Bold and code are spans of their own, and the link shows the host it opens after its words, as Details
  // and the chat draw a web link.
  page.window.pass();
  let tree = &page.window.tree;
  let drawn: Vec<String> = tree
    .resolve_quads(tree.last_layout().expect("a layout"))
    .into_iter()
    .flat_map(|quad| match quad.content {
      QuadContent::RichText { spans, .. } => spans.into_iter().map(|span| span.text).collect(),
      QuadContent::Text { text, .. } => vec![text],
      _ => Vec::new(),
    })
    .collect();
  for span in [
    "backoff and jitter",
    "delivery log",
    "example.com",
    "full jitter",
    "Retry-After",
  ] {
    assert!(drawn.iter().any(|text| text == span), "{span} in {drawn:?}");
  }
  // The brief stays 8 under its head (`Gt4az`).
  let mut bounds = |id: &str| {
    page
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  };
  let (head, text) = (bounds("task-brief-head"), bounds("task-brief-text"));
  assert!((text.y - (head.y + head.height) - 8.0).abs() < 0.5, "{head:?} {text:?}");
}
