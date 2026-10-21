//! Attachments (ORC-57): a copy of each file in `.orchester/attachments`, linked to its message with its name, kind,
//! size, hash and an image's size; kept once with a message retried under its key; forgotten with a chat message the
//! runner never took.

mod support;

use std::fs;

use orchester_core::{
  attachment::{AttachmentName, Dimensions, MediaType, NewAttachment},
  conversation::{Message, MessageBody, Recipient},
  id::{ChatId, ChatMessageId, IdempotencyKey},
  orchestrator::Chat,
  store::ConversationStore,
};
use orchester_store::{AttachmentOwner, attachment_file};
use support::{at, started_run};

/// A 3 by 2 PNG header.
fn png() -> Vec<u8> {
  let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
  bytes.extend(3u32.to_be_bytes());
  bytes.extend(2u32.to_be_bytes());
  bytes.extend([8, 6, 0, 0, 0]);
  bytes
}

fn file(name: &str, content: Vec<u8>) -> NewAttachment {
  NewAttachment::new(AttachmentName::new(name).expect("a name"), content).expect("a supported file")
}

fn body(text: &str) -> MessageBody {
  MessageBody::new(text).expect("text")
}

#[test]
fn a_task_message_keeps_a_copy_of_each_file_once() {
  let (repo, mut db, run, _execution) = started_run();
  let files = [file("shot.png", png()), file("notes.md", b"# Notes".to_vec())];
  let message = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Look"),
    at(10),
  );
  let key = IdempotencyKey::generate();
  db.send_message_with(key, &message, &files).expect("stored");
  // A retry under the same key returns the first message and keeps no second copy.
  let again = Message::from_person(
    run.task(),
    Some(run.id()),
    Recipient::Orchestrator,
    body("Look"),
    at(20),
  );
  assert_eq!(
    db.send_message_with(key, &again, &files).expect("again").id(),
    message.id()
  );
  let kept = db.attachments_of(AttachmentOwner::Message(message.id())).expect("read");
  assert_eq!(kept.len(), 2);
  assert_eq!(kept[0].name().as_str(), "shot.png");
  assert_eq!(kept[0].media(), MediaType::Png);
  assert_eq!(kept[0].dimensions(), Some(Dimensions { width: 3, height: 2 }));
  assert_eq!(kept[1].media(), MediaType::Text);
  assert_eq!(kept[1].size(), 7);
  let copy = attachment_file(repo.path(), &kept[1]);
  assert_eq!(fs::read(&copy).expect("the copy"), b"# Notes");
  assert!(copy.starts_with(repo.path().join(".orchester").join("attachments")));
  assert_eq!(db.message_attachments(run.task()).expect("by message").len(), 1);
  let folder = repo.path().join(".orchester").join("attachments");
  assert_eq!(fs::read_dir(&folder).expect("listed").count(), 2, "no second copy");
  let attached = db
    .attached_files(AttachmentOwner::Message(message.id()))
    .expect("files");
  assert_eq!(attached[0].path, attachment_file(repo.path(), &kept[0]));
  assert_eq!(
    db.attachment(kept[0].id()).expect("read").map(|found| found.attachment),
    Some(kept[0].clone())
  );
}

#[test]
fn a_chat_message_keeps_its_files_and_forgets_them_with_it() {
  let (repo, mut db, _run, _execution) = started_run();
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let started = Chat::start(chat, &body("shot.png"), at(10));
  let files = [file("shot.png", png())];
  assert!(
    db.keep_chat_message_with(&started, (message, &body("shot.png")), &files, at(10))
      .expect("kept")
  );
  // Kept before, it keeps the files it was kept with.
  assert!(
    !db
      .keep_chat_message_with(&started, (message, &body("shot.png")), &files, at(20))
      .expect("again")
  );
  let kept = db.chat_attachments(chat).expect("read");
  let attachments = kept.get(&message).expect("the message's files");
  assert_eq!(attachments.len(), 1);
  let copy = attachment_file(repo.path(), &attachments[0]);
  assert!(copy.is_file());
  db.forget_chat_message(message).expect("forgotten");
  assert!(!copy.exists(), "the copy goes with its message");
  assert!(
    db.attachments_of(AttachmentOwner::ChatMessage(message))
      .expect("read")
      .is_empty()
  );
}

#[test]
fn a_message_without_files_keeps_none() {
  let (repo, mut db, run, _execution) = started_run();
  let message = Message::from_person(run.task(), Some(run.id()), Recipient::Orchestrator, body("Hi"), at(10));
  db.send_message(IdempotencyKey::generate(), &message).expect("stored");
  assert!(
    db.attachments_of(AttachmentOwner::Message(message.id()))
      .expect("read")
      .is_empty()
  );
  assert!(!repo.path().join(".orchester").join("attachments").exists());
}
