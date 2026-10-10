//! The window's lifecycle (layer "App lifecycle" `Ob1P7`): what closing the window does with the agents
//! at work, and the runs the previous session left interrupted. Orchester has no background mode: the
//! runners and their agents do not outlive the window. A close with no agent at work stops the runners at
//! once (the status bar says so) and closes; with agents at work the close is held and the closing panel
//! takes the window's body — no dialog: the sidebar gives way, the tabs stay in place, dimmed, and do not
//! react, and Cancel brings back the page that was open, unchanged. What a close leads to is
//! `orchester_workspace::close::CloseFlow`; the runs interrupted by Quit now or a crash are offered on
//! their project's Overview ([`recovery`]).

mod keys;
mod panel;
pub mod recovery;
mod rows;

use lurq::{
  app::ctx::{Ctx, StreamEmitter},
  components::{Column, Row},
  core::Signal,
  node::{Element, HitTestBehavior, dimension::Dimension},
};
use orchester_workspace::close::{CloseFlow, ClosePhase};

pub use self::keys::{Choice, keys};
use self::panel::{ClosingPanel, PanelProps};
use crate::shell::status_bar::STATUS_BAR_HEIGHT;

/// The element id of the closing panel's layer over the window's body.
pub const CLOSING_ID: &str = "closing";

/// Hands the window's close requests (its ×, Alt+F4, Cmd+Q, the app's MCP) to `close`.
pub fn hook(ctx: &Ctx, close: &CloseFlow) {
  let close = close.clone();
  ctx
    .window()
    .on_close_requested(move |request| close.request(move || request.proceed()));
}

/// Where the close stands; reading it in a render renders again when it changes.
pub fn phase(ctx: &mut Ctx, close: &CloseFlow) -> ClosePhase {
  let phase = ctx.stream(
    close.clone(),
    |close, emitter: StreamEmitter<ClosePhase, ()>| async move {
      let mut updates = close.subscribe();
      loop {
        let phase = updates.borrow_and_update().clone();
        if !emitter.emit(phase) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  phase.state().get().data.unwrap_or(ClosePhase::Open)
}

/// The closing panel over the window's body while the close is held, `phase`; the status bar under it
/// stays visible and live. `focus` is the choice the keyboard moved to. Nothing otherwise.
pub fn overlay(ctx: &mut Ctx, phase: ClosePhase, close: &CloseFlow, focus: &Signal<Option<Choice>>) -> Option<Element> {
  if !phase.holds_window() {
    return None;
  }
  let props = PanelProps {
    phase,
    close: close.clone(),
    focus: focus.clone(),
  };
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .child(ctx.mount_keyed::<ClosingPanel>("closing-panel", props));
  let status_bar = Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(STATUS_BAR_HEIGHT))
    .hit_test(HitTestBehavior::None);
  Some(
    Column::new()
      .id(CLOSING_ID)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(body)
      .child(status_bar)
      .into(),
  )
}
