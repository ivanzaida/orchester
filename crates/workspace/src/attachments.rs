//! Attachments as the chats' composers take them (`orchester_core::attachment`): a file the person picked or dropped,
//! read only when it is a plain file — a folder or a shortcut is refused without reading what it points to, and a file
//! over the size limit without reading it — and checked for what it is; the refusals in the person's words; and the
//! agents of a task's run a message may reach, whose connections must take what it carries. Reading blocks on the file
//! system, so the UI calls it off the UI thread.

use std::{
  fs,
  io::Read,
  path::{Path, PathBuf},
};

use orchester_core::{
  attachment::{
    AttachmentKind, AttachmentName, AttachmentRefusal, NewAttachment,
    limits::{self, MAX_ATTACHMENT_BYTES},
    support,
  },
  connection::AgentProvider,
  conversation::Recipient,
  run::StepExecutor,
};

use crate::{messages::Message, tasks::detail::RunView};

/// Bytes in a megabyte, as the limits are said.
const MEGABYTE: u64 = 1024 * 1024;

/// A file or a message that was not taken, the name the person knows it by, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
  pub name: String,
  pub refusal: AttachmentRefusal,
}

impl Refused {
  pub fn new(name: impl Into<String>, refusal: AttachmentRefusal) -> Self {
    Self {
      name: name.into(),
      refusal,
    }
  }

  /// Why, in the person's words.
  pub fn message(&self) -> Message {
    let key = match self.refusal {
      AttachmentRefusal::Empty => "attachments.refused.empty",
      AttachmentRefusal::TooLarge { .. } => "attachments.refused.tooLarge",
      AttachmentRefusal::Unsupported => "attachments.refused.unsupported",
      AttachmentRefusal::Folder => "attachments.refused.folder",
      AttachmentRefusal::Shortcut => "attachments.refused.shortcut",
      AttachmentRefusal::Unreadable => "attachments.refused.unreadable",
      AttachmentRefusal::TooMany { .. } => "attachments.refused.tooMany",
      AttachmentRefusal::TotalTooLarge { .. } => "attachments.refused.totalTooLarge",
      AttachmentRefusal::NotTaken { kind, .. } => match kind {
        AttachmentKind::Image => "attachments.refused.notTaken.image",
        AttachmentKind::Pdf => "attachments.refused.notTaken.pdf",
        AttachmentKind::Text => "attachments.refused.notTaken.text",
      },
    };
    let message = Message::new(key).with("name", self.name.clone());
    match self.refusal {
      AttachmentRefusal::TooLarge { limit } | AttachmentRefusal::TotalTooLarge { limit } => {
        message.with("limit", (limit / MEGABYTE).to_string())
      }
      AttachmentRefusal::TooMany { limit } => message.with("limit", limit.to_string()),
      _ => message,
    }
  }
}

/// Why a plan's requested changes cannot carry attachments: they are words for the plan's agent only.
pub const fn plan_changes_refused() -> Message {
  Message::new("attachments.planChanges")
}

/// Why the stored copy of the attached file `name` could not be read back.
pub fn copy_unreadable(name: impl Into<String>) -> Message {
  Message::new("attachments.copyUnreadable").with("name", name.into())
}

/// The file at `path` the person picked or dropped, read and checked. A folder, or a link or shortcut to anything, is
/// refused without being followed; a file over [`MAX_ATTACHMENT_BYTES`] without being read.
pub fn read_file(path: &Path) -> Result<NewAttachment, Refused> {
  let shown = path.file_name().map_or_else(
    || path.display().to_string(),
    |name| name.to_string_lossy().into_owned(),
  );
  let refused = |refusal| Refused::new(shown.clone(), refusal);
  let name = AttachmentName::new(&shown).ok_or_else(|| refused(AttachmentRefusal::Unreadable))?;
  let metadata = fs::symlink_metadata(path).map_err(|_| refused(AttachmentRefusal::Unreadable))?;
  if metadata.is_dir() {
    return Err(refused(AttachmentRefusal::Folder));
  }
  if metadata.file_type().is_symlink() {
    return Err(refused(AttachmentRefusal::Shortcut));
  }
  limits::check_size(metadata.len()).map_err(refused)?;
  let mut content = Vec::new();
  fs::File::open(path)
    .and_then(|file| file.take(MAX_ATTACHMENT_BYTES + 1).read_to_end(&mut content))
    .map_err(|_| refused(AttachmentRefusal::Unreadable))?;
  NewAttachment::new(name, content).map_err(refused)
}

/// Each of `paths` read as [`read_file`] does, in order: what was taken, and what was refused with why.
pub fn read_files(paths: &[PathBuf]) -> (Vec<NewAttachment>, Vec<Refused>) {
  let mut taken = Vec::new();
  let mut refused = Vec::new();
  for path in paths {
    match read_file(path) {
      Ok(attachment) => taken.push(attachment),
      Err(why) => refused.push(why),
    }
  }
  (taken, refused)
}

/// `content`, pasted, as an attachment named `name`.
pub fn pasted(name: &str, content: Vec<u8>) -> Result<NewAttachment, Refused> {
  let named = AttachmentName::new(name).ok_or_else(|| Refused::new(name, AttachmentRefusal::Unreadable))?;
  NewAttachment::new(named, content).map_err(|refusal| Refused::new(name, refusal))
}

/// `held` with each of `new` added as long as the message's limits allow; the ones they refuse, with why.
pub fn add(held: &mut Vec<NewAttachment>, new: Vec<NewAttachment>) -> Vec<Refused> {
  let mut refused = Vec::new();
  for attachment in new {
    match limits::check_adding(held, &attachment) {
      Ok(()) => held.push(attachment),
      Err(refusal) => refused.push(Refused::new(attachment.name().as_str(), refusal)),
    }
  }
  refused
}

/// The connections of the agents of `run` a message to `recipient` may reach: every agent step's for Orchestrator,
/// which routes it to whichever agent works next, else the steps of the recipient's profile.
pub fn reachable_providers(run: &RunView, recipient: Recipient) -> Vec<AgentProvider> {
  let mut providers = Vec::new();
  for step in run.steps.iter().filter(|step| !step.tool_step) {
    let profile = match run.run.snapshot().step(&step.key).map(|pinned| pinned.executor()) {
      Some(StepExecutor::Agent { profile, .. }) => Some(profile.resource),
      _ => None,
    };
    let reached = match recipient {
      Recipient::Orchestrator => true,
      Recipient::Agent(to) => profile == Some(to),
    };
    if let (true, Some(provider)) = (reached, step.provider)
      && !providers.contains(&provider)
    {
      providers.push(provider);
    }
  }
  providers
}

/// Refuses `attachments` for a message to `recipient` in `run`, when the message's limits do not hold or an agent it
/// may reach could not read one of them. Without a run, the agents are not known yet: the runner names a file it could
/// not give when the message is delivered.
pub fn check_message(
  run: Option<&RunView>,
  recipient: Recipient,
  attachments: &[NewAttachment],
) -> Result<(), Refused> {
  let named = |refusal: AttachmentRefusal, at: usize| {
    let name = attachments
      .get(at)
      .map(|attachment| attachment.name().as_str().to_owned());
    Refused::new(name.unwrap_or_default(), refusal)
  };
  limits::check_message(attachments).map_err(|refusal| named(refusal, attachments.len().saturating_sub(1)))?;
  let Some(run) = run else {
    return Ok(());
  };
  for provider in reachable_providers(run, recipient) {
    if let Some(at) = attachments
      .iter()
      .position(|attachment| !support::takes(provider, attachment.media().kind()))
    {
      let kind = attachments[at].media().kind();
      return Err(named(AttachmentRefusal::NotTaken { kind, by: provider }, at));
    }
  }
  Ok(())
}
