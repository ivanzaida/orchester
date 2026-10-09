//! Background operations of the application, for the status bar: `orchester_workspace::activity`,
//! and the session's [`Activity`] as the root provides it to screens.

use lurq::app::ctx::Ctx;
pub use orchester_workspace::activity::*;

/// The session's activity, provided by the root.
pub fn of(ctx: &mut Ctx) -> Activity {
  ctx
    .use_context::<Activity>()
    .expect("the root provides Activity before mounting screens")
}
