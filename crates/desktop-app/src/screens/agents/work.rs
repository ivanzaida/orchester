//! A profile's writes off the UI thread: a fixed candidate is saved or published (`orchester_workspace::agents::
//! editor::apply`), and what was stored comes back as the page shows it. A save takes as allowed what the stored draft
//! already widens, which a person allowed when it was saved: an autosave asks only about what widens beyond it.
use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_workspace::{
  agents::{
    data::ProfileEntry,
    editor::{self, Outcome, Request},
  },
  background,
  messages::Message,
  tasks::outside::Outside,
};

use super::editor::Props;
use crate::services::Services;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
  pub request: Request,
  pub outcome: Outcome,
  /// The profile as stored after a write that saved or published.
  pub entry: Option<ProfileEntry>,
}
impl DevtoolsInspectable for Done {}

/// A candidate waiting for the person: what it widens, asked at the field that widens it, or publishing a stale draft
/// over the latest version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending(pub Option<Asked>);
impl DevtoolsInspectable for Pending {}

/// A fixed candidate and what its question lists: the widenings not allowed before it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
  pub request: Request,
  pub reasons: Vec<Message>,
}

/// The saved profile the page compares its fields with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved(pub Option<ProfileEntry>);
impl DevtoolsInspectable for Saved {}

/// The widenings the person allowed on this page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Allowed(pub Vec<Message>);
impl DevtoolsInspectable for Allowed {}

pub type Work = FutureAction<Request, Done, Message>;

pub fn work(ctx: &mut Ctx) -> Work {
  let root = ctx.props::<Props>().project.open.repo_root.clone();
  ctx.future_action(move |request: Request| {
    let root = root.clone();
    async move {
      background::run(move || {
        let mut request = request;
        if let Some(existing) = request.existing.as_ref() {
          for reason in editor::approved_in(&root, existing)? {
            if !request.allowed.contains(&reason) {
              request.allowed.push(reason);
            }
          }
        }
        let outcome = editor::apply(&root, &request)?;
        let entry = match &outcome {
          Outcome::Saved(id) => Some(editor::entry_in(&root, *id)?),
          Outcome::Confirm(_) => None,
        };
        Ok(Done {
          request,
          outcome,
          entry,
        })
      })
      .await?
    }
  })
}

/// The signals a write's end changes.
#[derive(Clone)]
pub struct Follow {
  pub saved: Signal<Saved>,
  pub pending: Signal<Pending>,
  /// Rests again once a write ended, so what was edited meanwhile is saved after it.
  pub again: Arc<dyn Fn() + Send + Sync>,
}

pub fn follow(ctx: &mut Ctx, work: &Work, follow: Follow) {
  let props = ctx.props::<Props>().clone();
  let changes = Services::of(ctx).changes;
  ctx.watch(&work.state(), move |state| {
    if state.is_pending() {
      return;
    }
    if let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) {
      match done.outcome {
        Outcome::Saved(id) => {
          follow.pending.set(Pending(None));
          follow.saved.set(Saved(done.entry.clone()));
          if done.request.existing.is_none() {
            props.nav.saved(id);
          }
          // Every other screen that shows the profiles reads them again; this one reads them again once.
          props
            .profiles
            .record(&changes, props.project.open.project.id(), Outside::Profiles);
        }
        Outcome::Confirm(reasons) => {
          let asked: Vec<Message> = reasons
            .iter()
            .filter(|reason| !done.request.allowed.contains(reason))
            .cloned()
            .collect();
          follow.pending.set(Pending(Some(Asked {
            request: Request {
              allowed: reasons,
              ..done.request
            },
            reasons: asked,
          })));
        }
      }
    }
    (follow.again)();
  });
}
