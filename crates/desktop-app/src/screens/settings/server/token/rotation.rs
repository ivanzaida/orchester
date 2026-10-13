//! *Rotate the token?* in place under the row, and what came of it: the new token in use, or the secret store's
//! refusal with *Try again*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::{Change, STORE_KEY, TokenRow, outcome, refused};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    card::{CardSurface, card},
    field::field_error,
    icon::Icon,
    inspector::note,
    message::message_text,
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
      None if state.is_fulfilled() => lines.push(
        outcome(
          "settings-mcp-token-rotated",
          Icon::CircleCheck,
          PaletteColor::Success,
          &ctx.t("settings.automation.server.token.rotated"),
        )
        .into(),
      ),
      None => {}
    }
    lines
  }

  /// *Rotate the token?* in place under the row.
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
      ButtonKind::Danger,
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
    card("settings-mcp-token-rotate-question", CardSurface::Decision)
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Tile.role())
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Loose.role())
      .child(
        Text::new(&ctx.t("settings.automation.server.token.rotateConfirm.title"))
          .variant(ExtraTypography::Strong.role()),
      )
      .child(note(
        "settings-mcp-token-rotate-agents",
        Icon::Unplug,
        &ctx.t("settings.automation.server.token.rotateConfirm.agents"),
      ))
      .child(note(
        "settings-mcp-token-rotate-sessions",
        Icon::Unlink,
        &ctx.t_args(
          "settings.automation.server.token.rotateConfirm.sessions",
          [("store", &*store)],
        ),
      ))
      .child(
        Row::new().spacing(SpacingSize::Sm).child(confirm).child(
          ActionButton::new(
            "settings-mcp-token-rotate-keep",
            ButtonKind::Secondary,
            ctx.t("settings.automation.server.token.rotateConfirm.cancel"),
          )
          .on_click(move || keep.set(false)),
        ),
      )
      .into()
  }

  /// That the secret store refused the new token, with *Try again*, which waits while a rotation runs.
  fn refused_line(
    &self,
    ctx: &Ctx,
    rotate: &Change<()>,
    reason: &orchester_workspace::messages::Message,
    pending: bool,
  ) -> Element {
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
}
