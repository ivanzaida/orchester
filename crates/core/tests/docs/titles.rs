use orchester_core::{
  docs::{Doc, DocError, DocSlug, DocSlugError, DocTitle, DocTitleError},
  id::DocId,
  time::Timestamp,
};

fn title(text: &str) -> DocTitle {
  DocTitle::new(text).expect("valid title")
}

fn slug_of(text: &str) -> String {
  DocSlug::from_title(&title(text)).as_str().to_owned()
}

#[test]
fn docs_title_is_trimmed_one_line_and_bounded() {
  assert_eq!(title("  Format and storage \t").as_str(), "Format and storage");
  assert_eq!(DocTitle::new(" \n "), Err(DocTitleError::Blank));
  assert_eq!(DocTitle::new("First\nSecond"), Err(DocTitleError::LineBreak));
  assert_eq!(
    DocTitle::new(&"a".repeat(201)),
    Err(DocTitleError::TooLong { length: 201 })
  );
  assert!(DocTitle::new(&"a".repeat(200)).is_ok());
}

#[test]
fn docs_slug_joins_lowercase_words_with_single_hyphens() {
  assert_eq!(slug_of("Format and storage"), "format-and-storage");
  assert_eq!(slug_of("D1 · Core & storage (non-UI)!"), "d1-core-storage-non-ui");
  assert_eq!(slug_of("Огляд проєкту"), "огляд-проєкту");
  assert_eq!(slug_of("!!! ???"), "untitled");
}

#[test]
fn docs_slug_of_a_long_title_stays_within_the_limit() {
  // `İ` lowercases to `i` and a combining dot, a gap between words: the slug would be twice the title's length.
  let slug = slug_of(&"İ".repeat(200));
  assert_eq!(slug.chars().count(), DocSlug::MAX_CHARS - 1);
  assert!(slug.starts_with("i-i-") && slug.ends_with("-i"));
}

#[test]
fn docs_slug_parse_takes_only_the_slug_form() {
  assert!(DocSlug::parse("format-and-storage").is_ok());
  for bad in ["", "Format", "two--hyphens", "-lead", "trail-", "with space"] {
    assert_eq!(
      DocSlug::parse(bad),
      Err(DocSlugError::Invalid(bad.to_owned())),
      "{bad:?}"
    );
  }
}

#[test]
fn docs_slug_unique_among_siblings_takes_the_next_free_number() {
  let slug = DocSlug::parse("plan").expect("valid slug");
  assert_eq!(slug.unique_among(&[]), slug);
  let taken = [slug.clone(), DocSlug::parse("plan-2").expect("valid slug")];
  assert_eq!(slug.unique_among(&taken).as_str(), "plan-3");
}

#[test]
fn docs_archived_doc_is_restored_and_archives_once() {
  let created = Timestamp::from_unix_millis(1_000);
  let notes = title("Notes");
  let mut doc = Doc::new(
    DocId::generate(),
    None,
    notes.clone(),
    DocSlug::from_title(&notes),
    0,
    created,
  );
  assert_eq!(
    doc.archive(Timestamp::from_unix_millis(999)),
    Err(DocError::ArchivedBeforeCreated(doc.id()))
  );
  doc.archive(Timestamp::from_unix_millis(2_000)).expect("archives");
  assert!(doc.is_archived());
  assert_eq!(
    doc.archive(Timestamp::from_unix_millis(3_000)),
    Err(DocError::Archived(doc.id()))
  );
  doc.restore().expect("restores");
  assert_eq!(doc.archived_at(), None);
  assert_eq!(doc.restore(), Err(DocError::NotArchived(doc.id())));
}
