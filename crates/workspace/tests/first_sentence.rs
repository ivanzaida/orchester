//! The first sentence of an agent's report (ORC-797): what the review page's summary row says of a run.
//! `cargo test -p orchester-workspace --test first_sentence`.

use orchester_workspace::display::first_sentence;

#[test]
fn first_sentence_is_the_first_one_of_a_plain_report() {
  assert_eq!(
    first_sentence("Capped the delay. Added a test for the bound.").as_deref(),
    Some("Capped the delay.")
  );
  assert_eq!(first_sentence("Done").as_deref(), Some("Done"));
}

#[test]
fn first_sentence_skips_headings_and_reads_the_paragraph_as_one_line() {
  let report =
    "## Cause\nThe export reads every\ncontact's deliveries one query at a time.\n\n## Fix\nRead them in a page.";
  assert_eq!(
    first_sentence(report).as_deref(),
    Some("The export reads every contact's deliveries one query at a time.")
  );
}

#[test]
fn first_sentence_does_not_end_inside_a_file_name_and_drops_markdown_marks() {
  assert_eq!(
    first_sentence("- **Fixed** `src/webhooks/sender.rs` and its test! Next.").as_deref(),
    Some("Fixed src/webhooks/sender.rs and its test!")
  );
}

#[test]
fn first_sentence_is_none_without_words() {
  assert_eq!(first_sentence(""), None);
  assert_eq!(first_sentence("## Only a heading\n\n---\n```\ncode.\n```"), None);
}
