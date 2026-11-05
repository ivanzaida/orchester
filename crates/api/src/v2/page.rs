//! Bounded direct reads (S10): a page request, the page it answers with, and the opaque cursor between them. Entities
//! come in stable ascending (creation sequence, id) order and histories in ascending sequence; every page of one
//! listing carries the same snapshot [`EventCursor`]. A page cursor is bound to the workspace, the filters, the sort
//! and the snapshot it was made for; once its snapshot is gone the server answers `cursor_expired`
//! ([`super::AuthorityErrorCode::CursorExpired`]).

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use super::EventCursor;

/// The page size a request without `limit` gets.
pub const DEFAULT_PAGE_LIMIT: u16 = 50;
/// The largest page a request may ask for.
pub const MAX_PAGE_LIMIT: u16 = 200;
/// How long, at least, a server keeps a listing's snapshot after it served one of its pages: each page served renews
/// it. A cursor used later may be refused with `cursor_expired`, and the client reads the listing again from its
/// first page.
pub const PAGE_CURSOR_RETENTION_SECONDS: u64 = 15 * 60;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("a page cursor cannot be empty")]
pub struct PageCursorError;

/// Where the next page of a listing starts. Opaque: clients send it back unchanged and never read it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PageCursor(String);

impl PageCursor {
  pub fn new(cursor: String) -> Result<Self, PageCursorError> {
    if cursor.is_empty() {
      return Err(PageCursorError);
    }
    Ok(Self(cursor))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl TryFrom<String> for PageCursor {
  type Error = PageCursorError;

  fn try_from(cursor: String) -> Result<Self, Self::Error> {
    Self::new(cursor)
  }
}

impl From<PageCursor> for String {
  fn from(cursor: PageCursor) -> Self {
    cursor.0
  }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PageRequestError {
  #[error("a page limit is between 1 and {MAX_PAGE_LIMIT}, got {0}")]
  LimitOutOfRange(u16),
}

/// `{after:PageCursor?, limit}`: the page after `after` (the first page without one), of at most `limit` items.
/// `limit` may be left out for [`DEFAULT_PAGE_LIMIT`] and is 1..=[`MAX_PAGE_LIMIT`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PageRequestWire", into = "PageRequestWire")]
pub struct PageRequest {
  after: Option<PageCursor>,
  limit: u16,
}

impl PageRequest {
  pub fn new(after: Option<PageCursor>, limit: Option<u16>) -> Result<Self, PageRequestError> {
    let limit = limit.unwrap_or(DEFAULT_PAGE_LIMIT);
    if limit == 0 || limit > MAX_PAGE_LIMIT {
      return Err(PageRequestError::LimitOutOfRange(limit));
    }
    Ok(Self { after, limit })
  }

  /// The first page, of [`DEFAULT_PAGE_LIMIT`] items.
  pub const fn first() -> Self {
    Self {
      after: None,
      limit: DEFAULT_PAGE_LIMIT,
    }
  }

  pub const fn after(&self) -> Option<&PageCursor> {
    self.after.as_ref()
  }

  pub const fn limit(&self) -> u16 {
    self.limit
  }
}

impl Default for PageRequest {
  fn default() -> Self {
    Self::first()
  }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageRequestWire {
  #[serde(default)]
  after: Option<PageCursor>,
  #[serde(default)]
  limit: Option<u16>,
}

impl TryFrom<PageRequestWire> for PageRequest {
  type Error = PageRequestError;

  fn try_from(wire: PageRequestWire) -> Result<Self, Self::Error> {
    Self::new(wire.after, wire.limit)
  }
}

impl From<PageRequest> for PageRequestWire {
  fn from(request: PageRequest) -> Self {
    Self {
      after: request.after,
      limit: Some(request.limit),
    }
  }
}

/// `{items, next:PageCursor?, cursor}`: one page of a listing; `next` is `null` on its last page, and `cursor` is the
/// listing's snapshot, the same on every page of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page<T> {
  pub items: Vec<T>,
  #[serde(deserialize_with = "nullable")]
  pub next: Option<PageCursor>,
  pub cursor: EventCursor,
}

/// A field the S10 reads always send, `null` when it has no value: unlike a plain `Option` field, leaving it out is
/// refused.
pub(crate) fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Option<T>, D::Error> {
  Option::<T>::deserialize(deserializer)
}
