//! Shared components mirroring the design's Foundations masters.

pub mod backdrop;
pub mod brand;
pub mod button;
pub mod cancel_dialog;
pub mod category_chip;
pub mod check_row;
pub mod command_chip;
pub mod command_row;
pub mod diff;
pub mod field;
pub mod form;
pub mod github_mark;
pub mod gutter;
pub mod icon;
pub mod link;
pub mod menu_option;
pub mod nav_item;
pub mod notice;
pub mod option_card;
pub mod organization_notice;
pub mod panel;
pub mod pressable;
pub mod property;
pub mod radio;
pub mod status_label;
pub mod summary_step;
pub mod switch;
pub mod tab;
pub mod unit_input;
pub mod value_box;

use std::sync::Arc;

use lurq::app::ctx::Ctx;

use crate::messages::Message;

pub fn message_text(ctx: &Ctx, message: &Message) -> Arc<str> {
  ctx.t_args(
    message.key,
    message.args.iter().map(|(name, value)| (*name, value.as_str())),
  )
}

/// The local day of `at`, e.g. "Oct 1".
pub fn day(ctx: &Ctx, at: orchester_core::time::Timestamp) -> Arc<str> {
  use chrono::Datelike;
  let Some(utc) = chrono::DateTime::from_timestamp_millis(at.unix_millis()) else {
    return ctx.t("sharing.unknownDay");
  };
  let local = utc.with_timezone(&chrono::Local).date_naive();
  let month = ctx.t(crate::display::MONTH_KEYS[local.month0() as usize]);
  ctx.t_args(
    "sharing.day",
    [("month", &*month), ("day", local.day().to_string().as_str())],
  )
}
