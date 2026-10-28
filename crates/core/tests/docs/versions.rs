use orchester_core::{
  blob::ContentHash,
  docs::{
    Commit, CommitMessage, CommitMessageError, CommitRefusal, DocAuthor, DocDraft, DocVersion, DocVersionNumber,
    StoredDocVersion, VersionNumberError, can_commit,
  },
  id::{DocId, ResourceId, RunId, StepExecutionId},
  memory::AgentProvenance,
  resource::StepKey,
  time::Timestamp,
};

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn number(value: u32) -> DocVersionNumber {
  DocVersionNumber::new(value).expect("not zero")
}

fn commit(by: DocAuthor) -> Commit {
  Commit {
    content_hash: ContentHash::new(&"a".repeat(64)).expect("valid hash"),
    message: Some(CommitMessage::new("  First notes ").expect("valid message")),
    committed_at: at(5_000),
    committed_by: by,
  }
}

fn agent() -> DocAuthor {
  DocAuthor::Agent(AgentProvenance {
    profile: ResourceId::generate(),
    run: RunId::generate(),
    step: StepKey::new("implement").expect("valid key"),
    execution: StepExecutionId::generate(),
  })
}

fn draft(doc: DocId, based_on: Option<DocVersionNumber>) -> DocDraft {
  DocDraft::new(doc, DocAuthor::Person, "# Notes\n".to_owned(), based_on, at(1_000))
}

fn version(doc: DocId, value: u32) -> DocVersion {
  DocVersion::from_stored(StoredDocVersion {
    doc,
    number: number(value),
    body: "# Notes\n".to_owned(),
    commit: commit(DocAuthor::Person),
  })
}

#[test]
fn docs_version_numbers_start_at_one_and_go_up_by_one() {
  assert_eq!(DocVersionNumber::new(0), Err(VersionNumberError::Zero));
  assert_eq!(DocVersionNumber::FIRST.get(), 1);
  assert_eq!(DocVersionNumber::FIRST.next(), Some(number(2)));
  assert_eq!(number(u32::MAX).next(), None);
  assert_eq!(number(4).to_string(), "v4");
}

#[test]
fn docs_first_commit_is_version_one_and_the_next_follows_it() {
  let doc = DocId::generate();
  let first = DocVersion::commit(&draft(doc, None), None, commit(DocAuthor::Person)).expect("commits");
  assert_eq!((first.doc(), first.number()), (doc, DocVersionNumber::FIRST));
  assert_eq!(first.body(), "# Notes\n");
  assert_eq!(first.message().map(CommitMessage::as_str), Some("First notes"));
  assert_eq!(first.committed_by(), &DocAuthor::Person);

  let by_agent = agent();
  let second = DocVersion::commit(
    &draft(doc, Some(first.number())),
    Some(&first),
    commit(by_agent.clone()),
  )
  .expect("commits");
  assert_eq!(second.number(), number(2));
  assert_eq!(second.committed_by(), &by_agent);
}

#[test]
fn docs_commit_of_a_draft_based_on_an_older_version_is_refused_with_both_numbers() {
  let doc = DocId::generate();
  let latest = version(doc, 5);
  assert_eq!(
    can_commit(&draft(doc, Some(number(4))), Some(&latest)),
    Err(CommitRefusal::Outdated {
      based_on: Some(number(4)),
      latest: number(5)
    })
  );
  assert_eq!(
    can_commit(&draft(doc, None), Some(&latest)),
    Err(CommitRefusal::Outdated {
      based_on: None,
      latest: number(5)
    })
  );
  assert_eq!(can_commit(&draft(doc, Some(number(5))), Some(&latest)), Ok(number(6)));
}

#[test]
fn docs_commit_refuses_a_base_the_doc_does_not_have_and_another_doc() {
  let doc = DocId::generate();
  assert_eq!(
    can_commit(&draft(doc, Some(number(1))), None),
    Err(CommitRefusal::UnknownBase { based_on: number(1) })
  );
  assert_eq!(
    can_commit(&draft(doc, Some(number(3))), Some(&version(doc, 2))),
    Err(CommitRefusal::UnknownBase { based_on: number(3) })
  );
  let other = DocId::generate();
  assert_eq!(
    can_commit(&draft(doc, Some(number(2))), Some(&version(other, 2))),
    Err(CommitRefusal::OtherDoc {
      draft: doc,
      latest: other
    })
  );
  assert_eq!(
    can_commit(&draft(doc, Some(number(u32::MAX))), Some(&version(doc, u32::MAX))),
    Err(CommitRefusal::NoNumberLeft(doc))
  );
}

#[test]
fn docs_draft_keeps_its_author_and_base_while_edited() {
  let doc = DocId::generate();
  let by_agent = agent();
  let mut draft = DocDraft::new(doc, by_agent.clone(), String::new(), Some(number(2)), at(1_000));
  draft.edit("More".to_owned(), at(2_000));
  assert_eq!((draft.body(), draft.updated_at()), ("More", at(2_000)));
  assert_eq!((draft.author(), draft.based_on()), (&by_agent, Some(number(2))));
}

#[test]
fn docs_commit_message_is_trimmed_and_not_blank() {
  assert_eq!(CommitMessage::new(" \n "), Err(CommitMessageError::Blank));
  assert_eq!(
    CommitMessage::new(&"m".repeat(2001)),
    Err(CommitMessageError::TooLong { length: 2001 })
  );
}
