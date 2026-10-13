//! Compare existing path components by opened object identity, using this directory's actual lookup rules.
use std::sync::Arc;

use orchester_core::integration::local::LocalPath;

use super::{LocalGitError, owned::Directory};

pub(super) fn collides(
  root: &Arc<Directory>,
  retained: &LocalPath,
  incoming: &LocalPath,
) -> Result<bool, LocalGitError> {
  let mut directory = Arc::clone(root);
  let mut retained = retained.as_str().split('/').peekable();
  let mut incoming = incoming.as_str().split('/').peekable();
  while let (Some(left), Some(right)) = (retained.next(), incoming.next()) {
    if left != right && !directory.aliases(left.as_ref(), right.as_ref())? {
      return Ok(false);
    }
    if retained.peek().is_none() || incoming.peek().is_none() {
      return Ok(true);
    }
    let Some(child) = directory.child(left.as_ref(), false)? else {
      return Ok(false);
    };
    directory = child;
  }
  Ok(false)
}
