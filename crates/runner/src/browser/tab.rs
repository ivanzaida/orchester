//! One of a run's tabs as the run's browser tools reach it ([`Tab`]): its commands go out on the browser's one
//! connection with the tab's session, its last `browser_read_page` named its elements with refs (`e12`), and its log
//! holds what it reported since it opened ([`TabLog`]).

use std::{collections::HashMap, sync::Arc, time::Duration};

use orchester_core::orchester_tools::browser::ElementRef;
use parking_lot::{Mutex, MutexGuard};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{
  BrowserError,
  cdp::{Cdp, CdpError, CdpEvent, SessionId, TargetId, TargetInfo},
  log::TabLog,
};

/// What the runner keeps of one tab besides its session.
#[derive(Default)]
pub(super) struct TabState {
  refs: Mutex<Refs>,
  pub(super) log: Mutex<TabLog>,
}

/// The refs of the tab's last read: each names an element by its CDP `backendNodeId`. Numbers go on from one read to
/// the next, so a ref of an earlier read names nothing and is reported stale rather than naming another element.
#[derive(Default)]
struct Refs {
  nodes: HashMap<String, i64>,
  last: u64,
}

/// One of a run's tabs, for one tool call.
pub struct Tab {
  target: TargetId,
  cdp: Arc<Cdp>,
  session: SessionId,
  state: Arc<TabState>,
}

impl Tab {
  pub(super) const fn new(target: TargetId, cdp: Arc<Cdp>, session: SessionId, state: Arc<TabState>) -> Self {
    Self {
      target,
      cdp,
      session,
      state,
    }
  }

  pub const fn id(&self) -> &TargetId {
    &self.target
  }

  /// Sends `method` with `params` to the tab and waits up to the connection's own limit for the answer.
  pub async fn call(&self, method: &str, params: Value) -> Result<Value, CdpError> {
    self.cdp.call(Some(&self.session), method, params).await
  }

  /// The tab's title and URL as the browser has them now.
  pub async fn info(&self) -> Result<TargetInfo, CdpError> {
    const METHOD: &str = "Target.getTargetInfo";
    let answer = self
      .cdp
      .call(None, METHOD, json!({ "targetId": self.target.as_str() }))
      .await?;
    let info = &answer["targetInfo"];
    let field = |field: &'static str| {
      info[field]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| CdpError::Unexpected {
          method: METHOD.to_owned(),
          field,
        })
    };
    Ok(TargetInfo {
      id: self.target.clone(),
      kind: field("type")?,
      title: field("title")?,
      url: field("url")?,
    })
  }

  /// [`Tab::call`] waiting up to `limit`.
  pub async fn call_within(&self, method: &str, params: Value, limit: Duration) -> Result<Value, CdpError> {
    self.cdp.call_within(Some(&self.session), method, params, limit).await
  }

  /// The tab's events from now on.
  pub fn events(&self) -> mpsc::UnboundedReceiver<CdpEvent> {
    self.cdp.subscribe(Some(self.session.clone()))
  }

  /// Navigates the tab to `url` and waits for its load.
  pub async fn load(&self, url: &str) -> Result<(), BrowserError> {
    super::running::load(&self.cdp, &self.session, url).await
  }

  /// Names `nodes` (`backendNodeId`s) with new refs, in order, replacing the refs of the tab's earlier read.
  pub fn name_refs(&self, nodes: &[i64]) -> Vec<String> {
    let mut refs = self.state.refs.lock();
    refs.nodes.clear();
    nodes
      .iter()
      .map(|node| {
        refs.last += 1;
        let name = format!("e{}", refs.last);
        refs.nodes.insert(name.clone(), *node);
        name
      })
      .collect()
  }

  /// The `backendNodeId` `element` names; stale when the tab's last read did not name it.
  pub fn node(&self, element: &ElementRef) -> Result<i64, BrowserError> {
    self
      .state
      .refs
      .lock()
      .nodes
      .get(element.as_str())
      .copied()
      .ok_or_else(|| BrowserError::StaleRef(element.as_str().to_owned()))
  }

  /// What the tab reported since it opened.
  pub fn log(&self) -> MutexGuard<'_, TabLog> {
    self.state.log.lock()
  }
}
