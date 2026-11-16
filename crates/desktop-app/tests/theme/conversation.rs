//! The Conversation's quiet approval row (`ARwiU`, ORC-353).
use orchester_app::theme::{ConversationSpacing, ConversationText};

use super::*;

/// A quiet approval row of the Conversation (`Event Â· switched to Manual` `ARwiU`, ORC-353): its lead in 12
/// `text_secondary` 1.3, its detail in 12 `text_muted` 1.4, 36 in past a message's avatar, in both modes.
#[test]
fn theme_conversation_approval_row_follows_the_design() {
  for mode in [Mode::Dark, Mode::Light] {
    let app = themed(mode);
    let theme = app.theme();
    for (role, line_height, color) in [
      (ConversationText::ApprovalLead, 1.3, PaletteColor::TextSecondary),
      (ConversationText::ApprovalDetail, 1.4, PaletteColor::TextMuted),
    ] {
      let style = theme.typography_style(role.role());
      assert_eq!(
        (style.font_size, style.weight, style.line_height, &*style.font_family),
        (12.0, FontWeight::Normal, line_height, "Geist"),
        "{role:?}"
      );
      assert_eq!(style.color, theme.palette_color(color), "{role:?} in {}", mode.name());
    }
    assert_eq!(
      theme.spacing_value(ConversationSpacing::ApprovalRow.role()),
      Dimension::Px(36.0)
    );
  }
}
