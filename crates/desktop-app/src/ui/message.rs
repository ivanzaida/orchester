//! The words of `orchester-workspace`'s [`Message`]s: a key of its locale files, loaded under its
//! namespace (`crate::i18n`), and the values of the key's placeholders.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::{i18n::NAMESPACE, messages::Message};

pub fn message_text(ctx: &Ctx, message: &Message) -> Arc<str> {
  ctx.t_ns_args(
    NAMESPACE,
    message.key,
    message.args.iter().map(|(name, value)| (*name, value.as_str())),
  )
}
