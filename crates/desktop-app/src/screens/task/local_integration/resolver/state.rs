use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::integration::local::Checkout;
use orchester_workspace::tasks::local_integration::ConflictView;

use super::candidate::CandidateState;
use crate::navigation::{Navigation, guard::LeaveGuard};

pub(super) type SourceIdentity = (ConflictView, Checkout, Option<u64>);
pub(super) type Seeded = Arc<parking_lot::Mutex<Option<SourceIdentity>>>;

#[derive(Clone)]
pub(in crate::screens::task::local_integration) struct EditorState {
  pub editor: CandidateState,
  pub selected: Signal<String>,
  pub seeded: Seeded,
  pub leave: LeaveGuard,
}
impl EditorState {
  pub fn new(ctx: &mut Ctx) -> Self {
    let editor = CandidateState::new();
    let observed = editor.clone();
    Self {
      editor,
      selected: ctx.signal(String::new()),
      seeded: Arc::new(parking_lot::Mutex::new(None)),
      leave: Navigation::of(ctx).guard(move || observed.dirty()),
    }
  }
}
