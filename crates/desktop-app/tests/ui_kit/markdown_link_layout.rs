//! lurq's Markdown with a link (ORC-71): a document that holds one link settles like one without, so a chat that
//! shows a reply with a link is not laid out again on every idle frame. lurq 0.49.1 left the window's layout dirty
//! after every pass while a link showed; 0.50.0 fixed it.

use lurq::components::{Markdown, MarkdownProps};

use super::harness::Kit;

/// The passes, of `PASSES` idle ones after the document was shown, that found the layout dirty.
fn dirty_passes(source: &'static str) -> usize {
  const PASSES: usize = 10;
  let mut kit = Kit::show(move |ctx| Markdown::mount(ctx, MarkdownProps::new(source)));
  for _ in 0..3 {
    kit.pass();
  }
  (0..PASSES)
    .filter(|_| kit.tree.pass_headless(&mut kit.app).reasons.layout_dirty)
    .count()
}

#[test]
fn markdown_link_layout_a_document_without_a_link_settles() {
  assert_eq!(dirty_passes("See the run, which is green."), 0);
}

#[test]
fn markdown_link_layout_a_document_with_a_link_settles() {
  assert_eq!(
    dirty_passes("See [the run](https://example.invalid/runs/12), which is green."),
    0,
    "a link leaves no layout dirty once shown"
  );
}
