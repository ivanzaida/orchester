//! *Rotate the token?* asked in place under the row (`Rotate` `HyIPf` in `shpsk`), and what came of it: the new token in
//! use, or the secret store's refusal with *Try again*. And the token the server cannot read (`Token unreadable`
//! `w9yATo`): what the store said, that the server is off until it can, *Try again* and *Rotate token*.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::messages::Message;

use super::{Change, Props, STORE_KEY, TokenRow, outcome, refused};
use crate::{
  mcp::ServerState,
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    notice::NoticeTone,
    setting_notice::SettingNotice,
  },
};

impl TokenRow {
  /// The question while it is asked, then what came of the rotation.
  pub(super) fn rotation_lines(&self, ctx: &Ctx, rotate: &Change<()>) -> Vec<Element> {
    let state = rotate.state().get();
    let mut lines = Vec::new();
    if self.confirming.get() {
      lines.push(self.confirmation(ctx, rotate.clone(), state.is_pending()));
    }
    if !self.rotated.get() {
      return lines;
    }
    match &state.error {
      Some(reason) => lines.push(self.refused_line(ctx, rotate, reason, state.is_pending())),
      None if state.is_fulfilled() => lines.push(outcome(
        "settings-mcp-token-rotated",
        &ctx.t("settings.automation.server.token.rotated"),
      )),
      None => {}
    }
    lines
  }

  /// *Rotate the token?* in place under the row: what rotating does, *Rotate token* and *Keep current token*.
  fn confirmation(&self, ctx: &Ctx, rotate: Change<()>, pending: bool) -> Element {
    let store = ctx.t(STORE_KEY);
    let (confirming, blocked, reveal, left, rotated) = (
      self.confirming.clone(),
      self.blocked.clone(),
      self.reveal.clone(),
      self.left.clone(),
      self.rotated.clone(),
    );
    let mut confirm = ActionButton::new(
      "settings-mcp-token-rotate-confirm",
      ButtonKind::Primary,
      ctx.t("settings.automation.server.token.rotateConfirm.confirm"),
    )
    .icon(Icon::RefreshCw);
    confirm = if pending {
      confirm.loading()
    } else {
      confirm.on_click(move || {
        if refused(&blocked) {
          return;
        }
        reveal.hide();
        left.set(0);
        confirming.set(false);
        rotated.set(true);
        rotate.run(());
      })
    };
    let keep = self.confirming.clone();
    SettingNotice::new("settings-mcp-token-rotate-question", NoticeTone::Warning)
      .title(ctx.t("settings.automation.server.token.rotateConfirm.title"))
      .body(ctx.t("settings.automation.server.token.rotateConfirm.agents"))
      .part(
        Text::new(&ctx.t_args(
          "settings.automation.server.token.rotateConfirm.sessions",
          [("store", &*store)],
        ))
        .id("settings-mcp-token-rotate-sessions")
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
      )
      .action(confirm)
      .action(
        ActionButton::new(
          "settings-mcp-token-rotate-keep",
          ButtonKind::Ghost,
          ctx.t("settings.automation.server.token.rotateConfirm.cancel"),
        )
        .on_click(move || keep.set(false)),
      )
      .into()
  }

  /// That the secret store refused the new token, with *Try again*, which waits while a rotation runs.
  fn refused_line(&self, ctx: &Ctx, rotate: &Change<()>, reason: &Message, pending: bool) -> Element {
    let store = ctx.t(STORE_KEY);
    let reason = message_text(ctx, reason);
    let mut again = ActionButton::new(
      "settings-mcp-token-rotate-again",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.tryAgain"),
    )
    .icon(Icon::RotateCcw);
    again = if pending {
      again.disabled()
    } else {
      let (rotate, blocked) = (rotate.clone(), self.blocked.clone());
      again.on_click(move || {
        if !refused(&blocked) {
          rotate.run(());
        }
      })
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        field_error(
          "settings-mcp-token-not-rotated",
          &ctx.t_args(
            "settings.automation.server.token.notRotated",
            [("store", &*store), ("reason", &*reason)],
          ),
        )
        .flex(1.0),
      )
      .child(again)
      .into()
  }

  /// While the server cannot read the token: what the secret store said, that the server is off until it can, *Try
  /// again* and *Rotate token*.
  pub(super) fn unreadable(&self, ctx: &Ctx, props: &Props, again: Change<()>, rotate: &Change<()>) -> Option<Element> {
    let ServerState::TokenUnreadable { reason } = &props.state else {
      return None;
    };
    let store = ctx.t(STORE_KEY);
    let reason = message_text(ctx, reason);
    let mut try_again = ActionButton::new(
      "settings-mcp-unreadable-again",
      ButtonKind::Secondary,
      ctx.t("settings.automation.server.token.tryAgain"),
    )
    .icon(Icon::RefreshCw);
    if props.server.is_none() || again.state().get().is_pending() {
      try_again = try_again.disabled();
    } else {
      let blocked = self.blocked.clone();
      try_again = try_again.on_click(move || {
        if !refused(&blocked) {
          again.run(());
        }
      });
    }
    let rotate_button = ActionButton::new(
      "settings-mcp-unreadable-rotate",
      ButtonKind::Ghost,
      ctx.t("settings.automation.server.token.rotate"),
    );
    let rotate_button = if props.server.is_some() && !self.confirming.get() && !rotate.state().get().is_pending() {
      rotate_button.on_click(self.ask_rotate())
    } else {
      rotate_button.disabled()
    };
    Some(
      SettingNotice::new("settings-mcp-unreadable", NoticeTone::Danger)
        .title(ctx.t_args("settings.automation.server.token.unreadable", [("store", &*store)]))
        .body(ctx.t_args(
          "settings.automation.server.token.unreadableBody",
          [("reason", &*reason)],
        ))
        .action(try_again)
        .action(rotate_button)
        .into(),
    )
  }
}
