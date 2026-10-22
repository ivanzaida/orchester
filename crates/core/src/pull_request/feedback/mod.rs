//! Agent feedback on a pull request (`docs/design/orchester-workspace.md`, "Pull request review and agent
//! feedback"): a person's notes for the agent of the pull request's linked task, kept in Orchester and never
//! posted to GitHub. A thread is anchored to lines of one side of a file at the revision it was written on, or to
//! the whole pull request, and holds the person's posts: the first request and the replies after it, each a
//! question (an explanation, no code changes) or a fix request (continue the task on its branch). A post reaches
//! the agent as a message of the task's conversation once Orchester sends it ([`FeedbackPost::message`]); until
//! then it is held and can be withdrawn. Resolving closes the thread in Orchester only.

mod anchor;
mod prompt;

use std::num::NonZeroU32;

use thiserror::Error;

pub use self::{
  anchor::{Located, locate},
  prompt::request_text,
};
use crate::{
  conversation::MessageBody,
  id::{FeedbackId, FeedbackPostId, MessageId, ResourceId, TaskId},
  repository::GitHubRepo,
  revision::Revision,
  time::Timestamp,
};

/// Why a feedback thread or post is not accepted.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FeedbackError {
  #[error("a line range ends at or after its start")]
  Span,
  #[error("an anchor keeps one line of code for each line it covers")]
  Excerpt,
  #[error("a file's path is not empty")]
  Path,
  #[error("a thread starts with a request")]
  NoPosts,
  #[error("the post is not in the thread")]
  UnknownPost,
  #[error("the post was sent already")]
  AlreadySent,
  #[error("the post was withdrawn")]
  Withdrawn,
  #[error("the thread is resolved")]
  Resolved,
  #[error("the thread is open")]
  NotResolved,
}

/// What a post asks of the agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeedbackKind {
  /// An explanation without changing code.
  Question,
  /// Continue the task on its branch and change the code.
  Fix,
}

impl FeedbackKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Question => "question",
      Self::Fix => "fix",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::Question, Self::Fix]
      .into_iter()
      .find(|kind| kind.as_str() == value)
  }
}

/// The side of a diff a line is numbered on: before the change, or after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffSide {
  Old,
  New,
}

impl DiffSide {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Old => "old",
      Self::New => "new",
    }
  }

  pub fn parse(value: &str) -> Option<Self> {
    [Self::Old, Self::New].into_iter().find(|side| side.as_str() == value)
  }
}

/// Lines `start` to `end` of a file, both included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineSpan {
  start: NonZeroU32,
  end: NonZeroU32,
}

impl LineSpan {
  pub fn new(start: NonZeroU32, end: NonZeroU32) -> Result<Self, FeedbackError> {
    if end < start {
      return Err(FeedbackError::Span);
    }
    Ok(Self { start, end })
  }

  /// One line.
  pub const fn line(line: NonZeroU32) -> Self {
    Self { start: line, end: line }
  }

  pub const fn start(self) -> u32 {
    self.start.get()
  }

  pub const fn end(self) -> u32 {
    self.end.get()
  }

  /// How many lines it covers.
  pub const fn len(self) -> u32 {
    self.end.get() - self.start.get() + 1
  }

  /// Always false: a span covers one line at least.
  pub const fn is_empty(self) -> bool {
    false
  }

  pub const fn contains(self, line: u32) -> bool {
    self.start.get() <= line && line <= self.end.get()
  }
}

/// Lines of one side of a file at a revision, with the code they held then, so the thread keeps what it was
/// about when later commits change or remove them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LineAnchor {
  path: String,
  side: DiffSide,
  span: LineSpan,
  excerpt: Vec<String>,
}

impl LineAnchor {
  pub fn new(path: &str, side: DiffSide, span: LineSpan, excerpt: Vec<String>) -> Result<Self, FeedbackError> {
    if path.trim().is_empty() {
      return Err(FeedbackError::Path);
    }
    if u32::try_from(excerpt.len()).ok() != Some(span.len()) {
      return Err(FeedbackError::Excerpt);
    }
    Ok(Self {
      path: path.to_owned(),
      side,
      span,
      excerpt,
    })
  }

  pub fn path(&self) -> &str {
    &self.path
  }

  pub const fn side(&self) -> DiffSide {
    self.side
  }

  pub const fn span(&self) -> LineSpan {
    self.span
  }

  /// The code of each line it covers, at the thread's revision.
  pub fn excerpt(&self) -> &[String] {
    &self.excerpt
  }
}

/// What a thread is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FeedbackAnchor {
  /// The whole pull request: no line, the revision only.
  PullRequest,
  Lines(LineAnchor),
}

impl FeedbackAnchor {
  pub const fn lines(&self) -> Option<&LineAnchor> {
    match self {
      Self::PullRequest => None,
      Self::Lines(lines) => Some(lines),
    }
  }
}

/// One request or reply of a thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackPost {
  id: FeedbackPostId,
  kind: FeedbackKind,
  body: MessageBody,
  /// The pull request's head when it was written: the code a fix applies to.
  revision: Revision,
  written_at: Timestamp,
  message: Option<MessageId>,
  withdrawn_at: Option<Timestamp>,
}

impl FeedbackPost {
  /// A new post, held until it is sent.
  pub fn new(kind: FeedbackKind, body: MessageBody, revision: Revision, now: Timestamp) -> Self {
    Self {
      id: FeedbackPostId::generate(),
      kind,
      body,
      revision,
      written_at: now,
      message: None,
      withdrawn_at: None,
    }
  }

  /// Rebuilds a stored post; a withdrawn one was never sent.
  pub fn restore(
    (id, kind, body): (FeedbackPostId, FeedbackKind, MessageBody),
    (revision, written_at): (Revision, Timestamp),
    (message, withdrawn_at): (Option<MessageId>, Option<Timestamp>),
  ) -> Result<Self, FeedbackError> {
    if message.is_some() && withdrawn_at.is_some() {
      return Err(FeedbackError::AlreadySent);
    }
    Ok(Self {
      id,
      kind,
      body,
      revision,
      written_at,
      message,
      withdrawn_at,
    })
  }

  pub const fn id(&self) -> FeedbackPostId {
    self.id
  }

  pub const fn kind(&self) -> FeedbackKind {
    self.kind
  }

  pub const fn body(&self) -> &MessageBody {
    &self.body
  }

  pub const fn revision(&self) -> &Revision {
    &self.revision
  }

  pub const fn written_at(&self) -> Timestamp {
    self.written_at
  }

  /// The conversation message it was sent as; `None` while it is held.
  pub const fn message(&self) -> Option<MessageId> {
    self.message
  }

  pub const fn withdrawn_at(&self) -> Option<Timestamp> {
    self.withdrawn_at
  }

  /// Held: neither sent nor withdrawn.
  pub const fn is_held(&self) -> bool {
    self.message.is_none() && self.withdrawn_at.is_none()
  }
}

/// What a thread is made of, checked by [`FeedbackThread::new`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadParts {
  pub id: FeedbackId,
  pub task: TaskId,
  pub repo: GitHubRepo,
  pub number: u64,
  pub anchor: FeedbackAnchor,
  /// The agent profile of the task's run the thread goes to.
  pub recipient: ResourceId,
  pub created_at: Timestamp,
  pub resolved_at: Option<Timestamp>,
}

/// A thread of agent feedback on a pull request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedbackThread {
  parts: ThreadParts,
  posts: Vec<FeedbackPost>,
}

impl FeedbackThread {
  /// A thread whose first post is `posts[0]`, the request it starts with.
  pub fn new(parts: ThreadParts, posts: Vec<FeedbackPost>) -> Result<Self, FeedbackError> {
    if posts.is_empty() {
      return Err(FeedbackError::NoPosts);
    }
    Ok(Self { parts, posts })
  }

  pub const fn id(&self) -> FeedbackId {
    self.parts.id
  }

  pub const fn task(&self) -> TaskId {
    self.parts.task
  }

  pub const fn repo(&self) -> &GitHubRepo {
    &self.parts.repo
  }

  pub const fn number(&self) -> u64 {
    self.parts.number
  }

  pub const fn anchor(&self) -> &FeedbackAnchor {
    &self.parts.anchor
  }

  pub const fn recipient(&self) -> ResourceId {
    self.parts.recipient
  }

  pub const fn created_at(&self) -> Timestamp {
    self.parts.created_at
  }

  pub const fn resolved_at(&self) -> Option<Timestamp> {
    self.parts.resolved_at
  }

  pub fn posts(&self) -> &[FeedbackPost] {
    &self.posts
  }

  /// The request it started with.
  pub fn first(&self) -> &FeedbackPost {
    &self.posts[0]
  }

  /// The latest post that was not withdrawn, else the first: what the thread asks now.
  pub fn current(&self) -> &FeedbackPost {
    self
      .posts
      .iter()
      .rev()
      .find(|post| post.withdrawn_at.is_none())
      .unwrap_or_else(|| self.first())
  }

  /// What the thread asks now.
  pub fn kind(&self) -> FeedbackKind {
    self.current().kind
  }

  /// The revision the thread's anchor was written on.
  pub fn revision(&self) -> &Revision {
    &self.first().revision
  }

  /// Every post was withdrawn: nothing of the thread reached the agent or will.
  pub fn is_withdrawn(&self) -> bool {
    self.posts.iter().all(|post| post.withdrawn_at.is_some())
  }

  pub fn post(&self, id: FeedbackPostId) -> Option<&FeedbackPost> {
    self.posts.iter().find(|post| post.id == id)
  }

  /// A person's next post; a resolved thread is reopened first.
  pub fn add(&mut self, post: FeedbackPost) -> Result<(), FeedbackError> {
    if self.parts.resolved_at.is_some() {
      return Err(FeedbackError::Resolved);
    }
    self.posts.push(post);
    Ok(())
  }

  /// Post `id`, held, was sent as conversation message `message`.
  pub fn sent(&mut self, id: FeedbackPostId, message: MessageId) -> Result<(), FeedbackError> {
    let post = self.held_mut(id)?;
    post.message = Some(message);
    Ok(())
  }

  /// Post `id`, held, is withdrawn at `at`: it never reaches the agent.
  pub fn withdraw(&mut self, id: FeedbackPostId, at: Timestamp) -> Result<(), FeedbackError> {
    let post = self.held_mut(id)?;
    post.withdrawn_at = Some(at);
    Ok(())
  }

  /// The person closes the thread in Orchester at `at`; the pull request is not approved or merged.
  pub fn resolve(&mut self, at: Timestamp) -> Result<(), FeedbackError> {
    if self.parts.resolved_at.is_some() {
      return Err(FeedbackError::Resolved);
    }
    self.parts.resolved_at = Some(at);
    Ok(())
  }

  pub fn reopen(&mut self) -> Result<(), FeedbackError> {
    if self.parts.resolved_at.take().is_none() {
      return Err(FeedbackError::NotResolved);
    }
    Ok(())
  }

  fn held_mut(&mut self, id: FeedbackPostId) -> Result<&mut FeedbackPost, FeedbackError> {
    let post = self
      .posts
      .iter_mut()
      .find(|post| post.id == id)
      .ok_or(FeedbackError::UnknownPost)?;
    if post.message.is_some() {
      return Err(FeedbackError::AlreadySent);
    }
    if post.withdrawn_at.is_some() {
      return Err(FeedbackError::Withdrawn);
    }
    Ok(post)
  }
}
