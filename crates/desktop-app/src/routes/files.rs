//! The code viewer's paths (`docs/design/code-viewer.md`, "The page"): `/project/<id>/files` shows the tree
//! with no file, `/project/<id>/files/<path>` a file, with `?at=` the version it is read at (left out for the
//! project's checkout) and `&line=` a range of lines (`12` or `12-20`). The file's path is percent-encoded
//! like every route value, its `/` kept as the path's own; a `/` that would leave an empty segment (a
//! leading one, as in an absolute path, or a doubled one) is encoded, so even a path the page refuses
//! reads back and the page can say why.

use orchester_workspace::files::{LineRange, Version};

use super::{
  RouteError,
  query::{Params, decode, encode, with_query},
};

pub(super) const FILES: &str = "files";
const AT: &str = "at";
const LINE: &str = "line";
/// The name of the path among the route's values, for an error that names it.
const PATH: &str = "path";

/// What the code viewer shows: a file or none, the version it is read at, and a range of its lines.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FilesView {
  /// The path as the link names it, checked by the page: a path it refuses is shown with the reason.
  file: Option<String>,
  at: Version,
  line: Option<LineRange>,
}

impl Default for FilesView {
  fn default() -> Self {
    Self {
      file: None,
      at: Version::Checkout,
      line: None,
    }
  }
}

impl FilesView {
  /// `file` at `at`; an empty path names no file.
  pub fn new(file: Option<&str>, at: Version) -> Self {
    Self {
      file: file.filter(|file| !file.is_empty()).map(str::to_owned),
      at,
      line: None,
    }
  }

  #[must_use]
  pub fn with_line(mut self, line: Option<LineRange>) -> Self {
    self.line = line;
    self
  }

  /// The same file and lines at `at`.
  #[must_use]
  pub fn at_version(mut self, at: Version) -> Self {
    self.at = at;
    self
  }

  pub fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  pub const fn at(&self) -> &Version {
    &self.at
  }

  pub const fn line(&self) -> Option<LineRange> {
    self.line
  }

  pub(super) fn path(&self, files: String) -> String {
    let path = match &self.file {
      Some(file) => format!("{files}/{}", encode_path(file)),
      None => files,
    };
    let at = (self.at != Version::Checkout).then(|| self.at.to_string());
    let line = self.line.map(|line| line.to_string());
    with_query(path, &[(AT, at.as_deref()), (LINE, line.as_deref())])
  }

  /// The view the segments after `files` and the query name.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
      parts.push(decode(segment).ok_or_else(|| RouteError::InvalidParam {
        param: PATH.to_owned(),
        value: (*segment).to_owned(),
      })?);
    }
    let file = parts.join("/");
    let at = match params.take(AT)? {
      Some(at) => at.parse().map_err(|_| RouteError::InvalidParam {
        param: AT.to_owned(),
        value: at,
      })?,
      None => Version::Checkout,
    };
    let line = match params.take(LINE)? {
      Some(line) => Some(line.parse().map_err(|_| RouteError::InvalidParam {
        param: LINE.to_owned(),
        value: line,
      })?),
      None => None,
    };
    Ok(Self::new(Some(&file), at).with_line(line))
  }
}

/// `file` percent-encoded with its `/` kept, except one that would leave an empty segment.
fn encode_path(file: &str) -> String {
  let mut encoded = String::with_capacity(file.len());
  let parts: Vec<&str> = file.split('/').collect();
  for (at, part) in parts.iter().enumerate() {
    if at > 0 {
      let empty_around = parts[at - 1].is_empty() || part.is_empty();
      if empty_around {
        encoded.push_str("%2F");
      } else {
        encoded.push('/');
      }
    }
    encoded.push_str(&encode(part));
  }
  encoded
}
