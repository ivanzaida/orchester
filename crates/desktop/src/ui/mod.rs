//! Shared components mirroring the design's Foundations masters.

pub mod backdrop;
pub mod brand;
pub mod button;
pub mod countdown;
pub mod field;
pub mod icon;
pub mod nav_item;
pub mod notice;
pub mod panel;
pub mod property;
pub mod status_label;
pub mod tab;

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use crate::messages::Message;

pub fn message_text(ctx: &Ctx, message: &Message) -> Arc<str> {
  ctx.t_args(
    message.key,
    message.args.iter().map(|(name, value)| (*name, value.as_str())),
  )
}
