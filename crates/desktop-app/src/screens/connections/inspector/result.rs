//! What a check pressed in the inspector (Test connection, Fetch models, Refresh models, Check again) does, in place
//! above the footer's actions: while it runs, what it asks and how long the CLI has to answer each question, or which
//! host the runner asks for a native connection; then whether it passed, what it found and when, which stays until the
//! next check. The footer's sentence above it says what to do. Drawn as Add connection's check row (`Check row`
//! `nL8Cy`): the inspector's design has no state of its own for it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::{ConnectionStatus, GitHubConnection, GitHubState},
  time::Timestamp,
};
use orchester_workspace::{connections::probe::COMMAND_TIMEOUT, display, messages::Message};

use super::{Inspecting, Tested};
use crate::{
  screens::connections::words::{CHECKING, Shown, Tone, agent_status, github_status},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
  },
};

/// The result box of a GitHub connection in `state`, once a check was pressed on it here.
pub(super) fn github(
  ctx: &Ctx,
  connection: &GitHubConnection,
  state: &GitHubState,
  inspecting: &Inspecting<'_>,
) -> Option<Element> {
  let said = match inspecting.tested? {
    Tested::Checking => checking(ctx, "connections.result.checkingGh", ""),
    Tested::Failed(error, at) => failed(ctx, error).at(at),
    Tested::Unread(error) => unread(ctx, error),
    Tested::Checked => {
      let at = connection.last_check()?.at;
      let shown = github_status(state);
      let detail = match state {
        GitHubState::SignedIn { account } => {
          ctx.t_args("connections.result.githubSignedIn", [("account", account.as_str())])
        }
        _ => ctx.t(shown.key),
      };
      Said::found(ctx, shown, detail, at)
    }
  };
  Some(said.render(ctx, inspecting.now))
}

fn checking(ctx: &Ctx, key: &str, program: &str) -> Said {
  let seconds = COMMAND_TIMEOUT.as_secs().to_string();
  Said {
    shown: CHECKING,
    heading: ctx.t(CHECKING.key),
    detail: ctx.t_args(key, [("program", program), ("seconds", seconds.as_str())]),
    at: None,
  }
}

fn failed(ctx: &Ctx, error: &Message) -> Said {
  let shown = agent_status(ConnectionStatus::CouldNotRun);
  Said {
    shown,
    heading: ctx.t("connections.result.failed"),
    detail: message_text(ctx, error),
    at: None,
  }
}

/// A check that ran, whose result reading the connections again could not show for `error`.
fn unread(ctx: &Ctx, error: &Message) -> Said {
  Said {
    heading: ctx.t("connections.result.unread"),
    ..failed(ctx, error)
  }
}

/// What the box says: the status's icon and tone, a heading, what was found, and when.
struct Said {
  shown: Shown,
  heading: Arc<str>,
  detail: Arc<str>,
  at: Option<Timestamp>,
}

impl Said {
  /// What a check pressed at `at` said, with its time at the right (`Time` `hFbL8` in 05g).
  fn at(self, at: Timestamp) -> Self {
    Self { at: Some(at), ..self }
  }

  /// A check that ran at `at` and ended in `shown`: passed, found a problem it names, or failed.
  fn found(ctx: &Ctx, shown: Shown, detail: Arc<str>, at: Timestamp) -> Self {
    let key = match shown.tone {
      Tone::Success => "connections.result.passed",
      Tone::Warning => "connections.result.attention",
      Tone::Danger | Tone::Muted | Tone::Busy => "connections.result.failed",
    };
    Self {
      shown,
      heading: ctx.t(key),
      detail,
      at: Some(at),
    }
  }

  fn render(self, ctx: &Ctx, now: Timestamp) -> Element {
    let time = self.at.map(|at| {
      Text::new(&message_text(ctx, &display::elapsed(now, at)))
        .id("connection-test-result-time")
        .variant(TypographyStyle::Caption)
        .nowrap()
    });
    // The heading's glyph says how the test went (05g, 05h, 05k): passed, found a problem, or failed.
    let glyph = match self.shown.tone {
      Tone::Success => Icon::CircleCheck,
      Tone::Warning => Icon::TriangleAlert,
      Tone::Danger | Tone::Muted => Icon::CircleX,
      Tone::Busy => self.shown.icon,
    };
    let top = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Base, self.shown.tone.color()))
      .child(
        Text::new(&self.heading)
          .id("connection-test-result-heading")
          .variant(TypographyStyle::Label)
          .color(self.shown.tone.word_color())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .with_children(time);
    Column::new()
      .id("connection-test-result")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
      .spacing(ExtraSpacing::Fine.role())
      .background(ExtraColor::InputSurface.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .describe("state", self.heading)
      .child(top)
      .child(
        Text::new(&self.detail)
          .id("connection-test-result-detail")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .into()
  }
}
