//! lurq 0.49.1's Markdown with a link (ORC-71), as small as it repeats: a document that holds one link leaves the
//! window's layout dirty after every pass, idle, so a window that shows it lays itself out again on every frame,
//! for as long as it is shown; the same document without a link settles at once. A chat that shows a reply with a
//! link is laid out on every frame. This is lurq's, reported with the numbers in `docs/plan.md` (ORC-71).

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
fn markdown_link_layout_a_link_leaves_the_layout_dirty_after_every_pass() {
  assert_eq!(
    dirty_passes("See [the run](https://example.invalid/runs/12), which is green."),
    10,
    "lurq lays the window out again on every idle pass while a link shows"
  );
}
