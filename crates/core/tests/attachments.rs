//! Attachments (ORC-57): what a file is by its content, the message's limits, which connections take which kinds,
//! and how agents are given them.

use std::fs;

use orchester_core::{
  attachment::{
    Attachment, AttachmentKind, AttachmentName, AttachmentRefusal, Dimensions, MediaType, NewAttachment, detect,
    input::{AttachedFile, claude_content, claude_line, codex_items, header, taken_text},
    is_names_only,
    limits::{self, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS, MAX_MESSAGE_BYTES},
    names_text, support,
  },
  blob::ContentHash,
  connection::AgentProvider,
  native::item::{Item, UserOrigin},
};
use serde_json::Value;

/// A 3 by 2 PNG header: signature, then an `IHDR` chunk.
fn png() -> Vec<u8> {
  let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
  bytes.extend(3u32.to_be_bytes());
  bytes.extend(2u32.to_be_bytes());
  bytes.extend([8, 6, 0, 0, 0]);
  bytes
}

fn name(name: &str) -> AttachmentName {
  AttachmentName::new(name).expect("a file name")
}

fn hash() -> ContentHash {
  ContentHash::new(&"a".repeat(64)).expect("64 hex digits")
}

#[test]
fn detect_tells_images_pdfs_and_text_by_content() {
  assert_eq!(
    detect(&png()),
    Ok((MediaType::Png, Some(Dimensions { width: 3, height: 2 })))
  );
  let gif = [b"GIF89a".as_slice(), &5u16.to_le_bytes(), &7u16.to_le_bytes()].concat();
  assert_eq!(
    detect(&gif),
    Ok((MediaType::Gif, Some(Dimensions { width: 5, height: 7 })))
  );
  let jpeg = [0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x04, 0x00, 0x09];
  assert_eq!(
    detect(&jpeg),
    Ok((MediaType::Jpeg, Some(Dimensions { width: 9, height: 4 })))
  );
  let mut webp = b"RIFF\0\0\0\0WEBPVP8X\0\0\0\0\0\0\0\0".to_vec();
  webp.extend([9, 0, 0, 4, 0, 0]);
  assert_eq!(
    detect(&webp),
    Ok((MediaType::Webp, Some(Dimensions { width: 10, height: 5 })))
  );
  assert_eq!(detect(b"%PDF-1.7\n..."), Ok((MediaType::Pdf, None)));
  assert_eq!(
    detect("fn main() {}\n// ünïcode".as_bytes()),
    Ok((MediaType::Text, None))
  );
  // A file named like an image but holding text is text.
  assert_eq!(detect(b"name,size\na,1\n"), Ok((MediaType::Text, None)));
}

#[test]
fn detect_refuses_binaries_and_shortcuts() {
  assert_eq!(detect(&[0, 1, 2, 3, 0xFF]), Err(AttachmentRefusal::Unsupported));
  assert_eq!(detect(&[0xC3, 0x28]), Err(AttachmentRefusal::Unsupported));
  let lnk = [
    0x4C, 0x00, 0x00, 0x00, 0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x46, 0x9B,
  ];
  assert_eq!(detect(&lnk), Err(AttachmentRefusal::Shortcut));
  assert_eq!(
    detect(b"book\0\0\0\0mark\0\0\0\0rest"),
    Err(AttachmentRefusal::Shortcut)
  );
}

#[test]
fn a_new_attachment_is_checked_and_never_shows_its_content() {
  assert_eq!(
    NewAttachment::new(name("empty.txt"), Vec::new()).err(),
    Some(AttachmentRefusal::Empty)
  );
  let big = vec![b'a'; usize::try_from(MAX_ATTACHMENT_BYTES).expect("fits") + 1];
  assert_eq!(
    NewAttachment::new(name("big.txt"), big).err(),
    Some(AttachmentRefusal::TooLarge {
      limit: MAX_ATTACHMENT_BYTES
    })
  );
  let secret = NewAttachment::new(name("notes.md"), b"token=hunter2".to_vec()).expect("text");
  assert_eq!(secret.text(), Some("token=hunter2"));
  assert!(!format!("{secret:?}").contains("hunter2"));
}

#[test]
fn names_keep_only_the_file_name() {
  assert_eq!(name("C:\\Users\\me\\shot.png").as_str(), "shot.png");
  assert_eq!(name("/home/me/a b.txt").as_str(), "a b.txt");
  assert!(AttachmentName::new("  ").is_none());
  assert!(AttachmentName::new("folder/").is_none());
}

#[test]
fn a_message_holds_ten_files_and_fifty_megabytes() {
  let file = |n: usize| NewAttachment::new(name(&format!("{n}.txt")), b"x".to_vec()).expect("text");
  let held: Vec<NewAttachment> = (0..MAX_ATTACHMENTS).map(file).collect();
  assert_eq!(
    limits::check_adding(&held, &file(99)),
    Err(AttachmentRefusal::TooMany { limit: MAX_ATTACHMENTS })
  );
  let size = usize::try_from(MAX_ATTACHMENT_BYTES).expect("fits");
  let large = |n: usize| NewAttachment::new(name(&format!("{n}.txt")), vec![b'a'; size]).expect("text");
  let held: Vec<NewAttachment> = (0..2).map(large).collect();
  assert_eq!(
    limits::check_adding(&held, &large(2)),
    Err(AttachmentRefusal::TotalTooLarge {
      limit: MAX_MESSAGE_BYTES
    })
  );
  assert_eq!(limits::check_message(&held), Ok(()));
  assert!(limits::is_long_paste(&"x".repeat(limits::LONG_PASTE_CHARS + 1)));
  assert!(!limits::is_long_paste(&"x".repeat(limits::LONG_PASTE_CHARS)));
}

#[test]
fn codex_takes_no_pdf_and_the_others_take_every_kind() {
  for kind in [AttachmentKind::Image, AttachmentKind::Pdf, AttachmentKind::Text] {
    assert!(support::takes(AgentProvider::ClaudeCodeCli, kind));
    assert!(support::takes(AgentProvider::AnthropicApi, kind));
  }
  assert!(support::takes(AgentProvider::CodexCli, AttachmentKind::Image));
  assert!(support::takes(AgentProvider::CodexCli, AttachmentKind::Text));
  assert!(!support::takes(AgentProvider::CodexCli, AttachmentKind::Pdf));
}

#[test]
fn a_message_of_attachments_only_is_kept_with_their_names() {
  let shot = Attachment::restore(
    orchester_core::id::AttachmentId::generate(),
    (name("shot.png"), MediaType::Png),
    (10, hash()),
    None,
  );
  let notes = Attachment::restore(
    orchester_core::id::AttachmentId::generate(),
    (name("notes.txt"), MediaType::Text),
    (4, hash()),
    None,
  );
  let both = [shot.clone(), notes];
  let text = names_text(both.iter().map(Attachment::name));
  assert_eq!(text, "shot.png\nnotes.txt");
  assert!(is_names_only(&text, &both));
  assert!(!is_names_only("Look at these", &both));
  assert!(!is_names_only("", &[]));
  assert!(shot.file_name().ends_with(".png"));
}

/// `files` kept in a temporary folder, as the project keeps them.
fn kept(folder: &std::path::Path, files: &[(&str, Vec<u8>)]) -> Vec<AttachedFile> {
  files
    .iter()
    .map(|(file, content)| {
      let new = NewAttachment::new(name(file), content.clone()).expect("a supported file");
      let attachment = Attachment::kept(&new, hash());
      let path = folder.join(attachment.file_name());
      fs::write(&path, content).expect("written");
      AttachedFile { attachment, path }
    })
    .collect()
}

#[test]
fn claude_code_gets_images_and_pdfs_as_blocks_and_text_as_text() {
  let folder = tempfile::tempdir().expect("a folder");
  let files = kept(
    folder.path(),
    &[
      ("shot.png", png()),
      ("spec.pdf", b"%PDF-1.4 body".to_vec()),
      ("notes.md", b"# Notes".to_vec()),
    ],
  );
  let content = claude_content("Look", &files).expect("readable copies");
  let kinds: Vec<&str> = content.iter().filter_map(|block| block["type"].as_str()).collect();
  assert_eq!(kinds, ["text", "text", "image", "text", "document", "text"]);
  assert_eq!(content[2]["source"]["media_type"], "image/png");
  assert_eq!(content[4]["source"]["media_type"], "application/pdf");
  assert!(content[5]["text"].as_str().expect("text").ends_with("# Notes"));
  let line: Value = serde_json::from_str(&claude_line("Look", &files).expect("a line")).expect("JSON");
  assert_eq!(line["type"], "user");
  let taken = taken_text(&content);
  assert!(taken.starts_with("Look\n[Attachment: shot.png (image/png"));
  assert!(taken.contains(&header(&files[1].attachment)));
}

#[test]
fn codex_gets_images_as_local_images_and_is_told_of_a_pdf_it_cannot_take() {
  let folder = tempfile::tempdir().expect("a folder");
  let files = kept(
    folder.path(),
    &[("shot.png", png()), ("spec.pdf", b"%PDF-1.4".to_vec())],
  );
  let items = codex_items("Look", &files).expect("readable copies");
  assert_eq!(items[2]["type"], "localImage");
  assert_eq!(items[2]["path"], files[0].path.to_string_lossy().as_ref());
  let pdf = items[3]["text"].as_str().expect("a note");
  assert!(pdf.contains("spec.pdf") && pdf.contains("PDF files"));
  assert_eq!(items.len(), 4);
}

#[test]
fn an_unreadable_copy_fails_the_input_instead_of_leaving_it_out() {
  let folder = tempfile::tempdir().expect("a folder");
  let files = kept(folder.path(), &[("shot.png", png())]);
  fs::remove_file(&files[0].path).expect("removed");
  assert!(claude_content("Look", &files).is_err());
  assert!(codex_items("Look", &files).is_err());
}

#[test]
fn native_agents_get_a_user_item_per_file() {
  let folder = tempfile::tempdir().expect("a folder");
  let files = kept(folder.path(), &[("shot.png", png()), ("notes.txt", b"hello".to_vec())]);
  let items = Item::attached(&files, UserOrigin::Task).expect("readable copies");
  let Item::User { text, attached, .. } = &items[0] else {
    panic!("a user item");
  };
  assert!(text.starts_with("[Attachment: shot.png"));
  assert_eq!(
    attached.as_ref().map(|file| file.attachment.media()),
    Some(MediaType::Png)
  );
  let Item::User { text, attached, .. } = &items[1] else {
    panic!("a user item");
  };
  assert!(text.ends_with("\nhello") && attached.is_none());
  // Kept in the native conversation as JSON, and read back the same.
  let json = serde_json::to_string(&items[0]).expect("JSON");
  assert_eq!(serde_json::from_str::<Item>(&json).expect("read back"), items[0]);
  let old: Item = serde_json::from_str(r#"{"kind":"user","text":"Do it","origin":"task"}"#).expect("an older item");
  assert_eq!(old, Item::user("Do it", UserOrigin::Task));
}
