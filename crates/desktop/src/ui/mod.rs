//! Shared components mirroring the design's Foundations masters.

pub mod backdrop;
pub mod brand;
pub mod button;
pub mod check_row;
pub mod command_chip;
pub mod command_row;
pub mod diff;
pub mod field;
pub mod form;
pub mod github_mark;
pub mod icon;
pub mod link;
pub mod nav_item;
pub mod notice;
pub mod option_card;
pub mod panel;
pub mod pressable;
pub mod property;
pub mod radio;
pub mod status_label;
pub mod summary_step;
pub mod switch;
pub mod tab;
pub mod unit_input;

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use crate::messages::Message;

pub fn message_text(ctx: &Ctx, message: &Message) -> Arc<str> {
  ctx.t_args(
    message.key,
    message.args.iter().map(|(name, value)| (*name, value.as_str())),
  )
}
