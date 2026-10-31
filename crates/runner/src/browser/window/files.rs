//! A page's request for files (`docs/design/browser.md`, "What frames do not show"), which the browser hands over
//! instead of showing a chooser (`Page.setInterceptFileChooserDialog`, set as a tab opens). On the person's tab the
//! request is told to the window, which opens the system's own file dialog, and kept here until the window's answer
//! sets the chosen files on the field that asked (`DOM.setFileInputFiles`), as the page's own chooser would; a tab that
//! navigates or closes first drops it. An agent's tab never comes here: the browser cancels its chooser, and the tab's
//! log tells the agent (`crate::browser::log`).

use std::{
  collections::HashMap,
  path::PathBuf,
  sync::atomic::{AtomicU64, Ordering},
};

use orchester_ipc::browser::{BrowserEvent, FileChooser, FileChooserId};
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{Window, follow::Followed, hit, tab_id};
use crate::browser::{
  BrowserError,
  cdp::{Cdp, SessionId, TargetId},
};

/// Reads the field's `accept` and its accessible name: what labels it by id, its own label, its `<label>`s, its title.
const READ_FIELD: &str = "function() {
  const text = (node) => (node ? node.textContent : '');
  const by = (this.getAttribute('aria-labelledby') || '').split(/\\s+/).filter(Boolean);
  const named = by.map((id) => text(document.getElementById(id))).join(' ')
    || this.getAttribute('aria-label')
    || Array.from(this.labels || [], text).join(' ')
    || this.title
    || '';
  return { accept: this.accept || '', label: named.replace(/\\s+/g, ' ').trim() };
}";

/// A request of a tab's field for files, until the window answers it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Asked {
  id: FileChooserId,
  /// The browser's id of the field's node.
  node: u64,
  multiple: bool,
}

/// The request each of the person's tabs makes for files, at most one: a later request of a tab replaces its earlier
/// one, whose answer then comes too late.
#[derive(Default)]
pub(super) struct FileRequests {
  asked: Mutex<HashMap<TargetId, Asked>>,
  numbers: AtomicU64,
}

impl FileRequests {
  fn ask(&self, target: TargetId, node: u64, multiple: bool) -> FileChooserId {
    let id = FileChooserId::new(self.numbers.fetch_add(1, Ordering::Relaxed) + 1);
    self.asked.lock().insert(target, Asked { id, node, multiple });
    id
  }

  /// The request `id` of `target`, which is answered now; `None` when the tab no longer makes it.
  pub(super) fn take(&self, target: &TargetId, id: FileChooserId) -> Option<Asked> {
    let mut asked = self.asked.lock();
    let request = *asked.get(target).filter(|request| request.id == id)?;
    asked.remove(target);
    Some(request)
  }

  /// `target` navigated or closed: its request is dropped.
  pub(super) fn forget(&self, target: &TargetId) {
    self.asked.lock().remove(target);
  }

  /// The browser starts or ended: no tab asks any more.
  pub(super) fn clear(&self) {
    self.asked.lock().clear();
  }
}

/// The person's `tab` asks for files, as the browser's `Page.fileChooserOpened` `params` say: the request is kept and
/// told to the window, with what the field accepts and its name as the page has them.
pub(super) async fn asked(cdp: &Cdp, tab: &Followed, window: &Window, params: &Value) {
  let Some(node) = params.get("backendNodeId").and_then(Value::as_u64) else {
    tracing::warn!("a page asked for files without naming its field; the request is left unanswered");
    return;
  };
  let multiple = params.get("mode").and_then(Value::as_str) == Some("selectMultiple");
  let (accept, label) = match hit::call_on(cdp, &tab.session, node, READ_FIELD, &[]).await {
    Ok(read) => described(&read),
    Err(error) => {
      // The dialog still opens, for any file and without the field's name.
      tracing::debug!(%error, "the file field's types and name could not be read");
      (Vec::new(), None)
    }
  };
  let id = window.files.ask(tab.target.clone(), node, multiple);
  window.send(BrowserEvent::FileChooserOpened {
    tab: tab_id(&tab.target),
    chooser: FileChooser {
      id,
      multiple,
      accept,
      label,
    },
  });
}

/// The field's accepted types, one per entry of its `accept`, and its name when it has one.
fn described(read: &Value) -> (Vec<String>, Option<String>) {
  let accept = read["accept"]
    .as_str()
    .unwrap_or_default()
    .split(',')
    .map(str::trim)
    .filter(|kind| !kind.is_empty())
    .map(str::to_owned)
    .collect();
  let label = read["label"]
    .as_str()
    .filter(|label| !label.is_empty())
    .map(str::to_owned);
  (accept, label)
}

/// Sets `paths` on the field `asked` of the tab of `session`, as its own chooser would, with the events a page listens
/// to; refused for no file, for more files than the field takes, and for a path the browser cannot be given.
pub(super) async fn set(cdp: &Cdp, session: &SessionId, asked: Asked, paths: &[PathBuf]) -> Result<(), BrowserError> {
  if paths.is_empty() {
    return Err(BrowserError::Element(
      "no file was chosen for the file field".to_owned(),
    ));
  }
  if !asked.multiple && paths.len() > 1 {
    return Err(BrowserError::Element(format!(
      "the file field takes one file, not {}",
      paths.len()
    )));
  }
  let files = paths
    .iter()
    .map(|path| {
      path
        .to_str()
        .ok_or_else(|| BrowserError::Element(format!("{} is no path the browser can open", path.display())))
    })
    .collect::<Result<Vec<&str>, _>>()?;
  cdp
    .call(
      Some(session),
      "DOM.setFileInputFiles",
      json!({ "files": files, "backendNodeId": asked.node }),
    )
    .await?;
  Ok(())
}
