//! What a check pressed in the inspector (Test connection, Refresh models, Check again) does, in place above the
//! footer's actions: while it runs, what it asks and how long the CLI has to answer each question; then whether it
//! passed, what it found and when, which stays until the next check. The footer's sentence above it says what to do.
//! Drawn as Add connection's check row (`Check row` `nL8Cy`): the inspector's design has no state of its own for it.

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
  connection::{AgentConnection, ConnectionStatus, GitHubConnection, GitHubState},
  time::Timestamp,
};
use orchester_workspace::{connections::probe::COMMAND_TIMEOUT, display, messages::Message};

use super::{Inspecting, Tested};
use crate::{
  screens::connections::words::{CHECKING, Shown, Tone, agent_status, checked_version, github_status, provider_key},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{icon::icon, message::message_text},
};

/// The result box of an agent connection, once a check was pressed on it here.
pub(super) fn agent(ctx: &Ctx, connection: &AgentConnection, inspecting: &Inspecting<'_>) -> Option<Element> {
  let program = ctx.t(provider_key(connection.provider()));
  let said = match inspecting.tested? {
    Tested::Checking => checking(ctx, "connections.result.checkingCli", &program),
    Tested::Failed(error) => failed(ctx, error),
    Tested::Unread(error) => unread(ctx, error),
    Tested::Checked => {
      let at = connection.last_check()?.at;
      let status = connection.status_with(&inspecting.listed.secrets);
      let version = checked_version(connection).unwrap_or_default();
      let (models, seconds) = (
        connection.models().len().to_string(),
        COMMAND_TIMEOUT.as_secs().to_string(),
      );
      let args = [
        ("program", &*program),
        ("version", version),
        ("models", models.as_str()),
        ("seconds", seconds.as_str()),
      ];
      let key = agent_key(status, connection.api_provider().is_some());
      Said::found(ctx, agent_status(status), ctx.t_args(key, args), at)
    }
  };
  Some(said.render(ctx, inspecting.now))
}

/// The result box of a GitHub connection in `state`, once a check was pressed on it here.
pub(super) fn github(
  ctx: &Ctx,
  connection: &GitHubConnection,
  state: &GitHubState,
  inspecting: &Inspecting<'_>,
) -> Option<Element> {
  let said = match inspecting.tested? {
    Tested::Checking => checking(ctx, "connections.result.checkingGh", ""),
    Tested::Failed(error) => failed(ctx, error),
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

/// What a check that ended in `status` found, for a CLI on an API provider when `on_provider`.
const fn agent_key(status: ConnectionStatus, on_provider: bool) -> &'static str {
  match status {
    ConnectionStatus::Connected if on_provider => "connections.result.verifiedProvider",
    ConnectionStatus::Connected => "connections.result.verified",
    ConnectionStatus::SignInRequired => "connections.result.signedOut",
    ConnectionStatus::SignInUnknown => "connections.result.signInUnknown",
    ConnectionStatus::KeyMissing => "connections.result.keyMissing",
    ConnectionStatus::ExecutableMissing => "connections.result.notFound",
    ConnectionStatus::NotRecognized => "connections.result.notRecognized",
    ConnectionStatus::NotChecked
    | ConnectionStatus::CouldNotRun
    | ConnectionStatus::KeyRejected
    | ConnectionStatus::BillingProblem
    | ConnectionStatus::EndpointUnreachable => "connections.result.couldNotRun",
  }
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
    let top = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Pair.role())
      .align_items(Alignment::Center)
      .child(icon(self.shown.icon, IconSize::Base, self.shown.tone.color()))
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
      .background(PaletteColor::SurfaceInput)
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
