//! How Settings › Orchester says things: a refusal or failure as its own words (`Said`, a key of the `cloud` area
//! and its values), a server's connection as an icon and a word in the state's colors (`Aowlk`), and the cloud's
//! and the team servers' refusals in the person's words.

use lurq::app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor};
use orchester_api::{ErrorCode, client::ClientError, cloud::CloudErrorCode};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  display,
  sharing::{Connection, ServerEntry, SharingError, cloud::CloudError},
};

use crate::ui::icon::Icon;

/// Words to show: a key and its values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
  pub key: &'static str,
  pub args: Vec<(&'static str, String)>,
}

impl DevtoolsInspectable for Said {}

impl Said {
  pub fn new(key: &'static str) -> Self {
    Self { key, args: Vec::new() }
  }

  pub fn with(mut self, name: &'static str, value: impl Into<String>) -> Self {
    self.args.push((name, value.into()));
    self
  }

  pub fn text(&self, ctx: &Ctx) -> String {
    ctx
      .t_args(self.key, self.args.iter().map(|(name, value)| (*name, value.as_str())))
      .to_string()
  }
}

/// What a server's row and header show for its state.
pub struct Look {
  pub icon: Icon,
  pub icon_color: PaletteColor,
  pub key: &'static str,
  pub color: PaletteColor,
  /// The state's name for the element tree.
  pub name: &'static str,
}

/// The state of `entry`: a changed certificate before anything else, then its connection.
pub fn server_look(entry: &ServerEntry) -> Look {
  let look = |icon, icon_color, key, color, name| Look {
    icon,
    icon_color,
    key,
    color,
    name,
  };
  if entry.certificate_changed {
    return look(
      Icon::ShieldAlert,
      PaletteColor::Danger,
      "cloud.state.certificateChanged",
      PaletteColor::Danger,
      "certificate-changed",
    );
  }
  match entry.connection {
    None | Some(Connection::SignedOut) => look(
      Icon::LogOut,
      PaletteColor::Warning,
      "cloud.state.signedOut",
      PaletteColor::Warning,
      "signed-out",
    ),
    Some(Connection::Checking) => look(
      Icon::LoaderCircle,
      PaletteColor::TextMuted,
      "cloud.state.connecting",
      PaletteColor::TextSecondary,
      "connecting",
    ),
    Some(Connection::Connected) => look(
      Icon::CircleCheck,
      PaletteColor::Success,
      "cloud.state.connected",
      PaletteColor::TextSecondary,
      "connected",
    ),
    Some(Connection::Reconnecting { .. }) => look(
      Icon::LoaderCircle,
      PaletteColor::TextMuted,
      "cloud.state.connecting",
      PaletteColor::TextSecondary,
      "reconnecting",
    ),
    Some(Connection::Offline { .. }) => look(
      Icon::WifiOff,
      PaletteColor::Warning,
      "cloud.state.offline",
      PaletteColor::Warning,
      "offline",
    ),
  }
}

/// What to tell the person about `error` from Orchester Cloud.
pub fn cloud_error(error: &CloudError) -> Said {
  match error {
    CloudError::Client(error) => cloud_client(error),
    CloudError::Store(_) | CloudError::Vault(_) | CloudError::Interrupted => Said::new("cloud.errors.local"),
    CloudError::Address(_) => Said::new("cloud.errors.address"),
    CloudError::SignedOut => Said::new("cloud.errors.signedOut"),
    CloudError::JoinRefused => Said::new("cloud.errors.joinRefused"),
    CloudError::TooManyAsks => Said::new("cloud.errors.tooManyAsks"),
    CloudError::InviteEnded => Said::new("cloud.invites.ended"),
    CloudError::NoServer => Said::new("cloud.errors.noServer"),
    CloudError::Join(error) => sharing_error(error),
  }
}

fn cloud_client(error: &ClientError) -> Said {
  match error {
    ClientError::Cloud { code, retry_after, .. } => match code {
      CloudErrorCode::InvalidCredentials => Said::new("cloud.signIn.wrong"),
      CloudErrorCode::RateLimited => minutes(Said::new("cloud.errors.rateLimited"), *retry_after),
      CloudErrorCode::Busy => Said::new("cloud.errors.busy"),
      CloudErrorCode::KeysUnavailable => Said::new("cloud.errors.keys"),
      _ => Said::new("cloud.errors.refused"),
    },
    error if error.is_connection() => Said::new("cloud.errors.unreachable"),
    ClientError::UntrustedCertificate => Said::new("cloud.errors.certificate"),
    _ => Said::new("cloud.errors.refused"),
  }
}

/// What to tell the person about `error` from a team server.
pub fn sharing_error(error: &SharingError) -> Said {
  match error {
    SharingError::Client(error) => server_client(error),
    SharingError::SignedOut => Said::new("cloud.errors.serverSignedOut"),
    SharingError::CertificateChanged => Said::new("cloud.errors.certificateChanged"),
    SharingError::Offline => Said::new("cloud.errors.serverUnreachable"),
    SharingError::ForeignAddress { own } => {
      Said::new("sharing.upload.foreignAddress").with("own", shown_address(own.as_str()))
    }
    _ => Said::new("cloud.errors.local"),
  }
}

/// A team server's refusal, or why it could not be asked.
pub fn server_client(error: &ClientError) -> Said {
  match error {
    ClientError::Api { code, retry_after, .. } => match code {
      ErrorCode::InvalidCode => Said::new("sharing.connect.code.invalid"),
      ErrorCode::UsernameTaken => Said::new("sharing.connect.username.taken"),
      ErrorCode::Invalid => Said::new("sharing.connect.username.needed"),
      ErrorCode::RateLimited => minutes(Said::new("sharing.connect.code.rateLimited"), *retry_after),
      ErrorCode::TooManyCodes => Said::new("cloud.errors.tooManyCodes"),
      ErrorCode::NotInvited => Said::new("sharing.connect.notInvited"),
      ErrorCode::CloudOff => Said::new("sharing.connect.cloudOff"),
      ErrorCode::KeySetStale => Said::new("sharing.connect.keySetStale"),
      ErrorCode::ApprovalRefused => Said::new("sharing.connect.approvalRefused"),
      ErrorCode::InvalidIdentity => Said::new("sharing.connect.invalidIdentity"),
      _ => Said::new("cloud.errors.serverRefused"),
    },
    ClientError::Cloud { .. } => cloud_client(error),
    ClientError::UntrustedCertificate => Said::new("cloud.errors.certificateChanged"),
    ClientError::PlainHttpRefused => Said::new("sharing.connect.address.plain"),
    ClientError::NotOrchester { .. } | ClientError::Redirect { .. } => {
      Said::new("sharing.connect.address.notOrchester")
    }
    ClientError::Address(_) => Said::new("sharing.connect.address.invalid"),
    ClientError::TimedOut | ClientError::Unreachable => Said::new("cloud.errors.serverUnreachable"),
    ClientError::Setup => Said::new("cloud.errors.local"),
  }
}

/// `said` with how many minutes to wait, at least one.
fn minutes(said: Said, retry_after: Option<u64>) -> Said {
  let minutes = retry_after.map_or(15, |seconds| seconds.div_ceil(60).max(1));
  said.with("minutes", minutes.to_string())
}

/// A project's address as lists show it: without `https://`, `orchester.kontur.dev/billing`; a plain HTTP address
/// keeps its scheme, so it never reads as an encrypted one.
pub fn shown_address(address: &str) -> &str {
  address.strip_prefix("https://").unwrap_or(address)
}

/// A day as the design writes it, `Oct 14`.
pub fn day(ctx: &Ctx, at: Timestamp) -> String {
  let (month, day) = display::month_day(at);
  let month = ctx
    .t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month])
    .to_string();
  ctx
    .t_args("cloud.day", [("month", month), ("day", day.to_string())])
    .to_string()
}

/// A day and its clock time, `Oct 14, 09:12`.
pub fn day_and_time(ctx: &Ctx, at: Timestamp) -> String {
  ctx
    .t_args("cloud.dayTime", [("day", day(ctx, at)), ("time", display::clock(at))])
    .to_string()
}

/// When something happened: its clock time today (`14:32`), else its day and time (`Oct 14, 09:12`).
pub fn moment(ctx: &Ctx, at: Timestamp) -> String {
  if at.local_day_start() == Timestamp::now().local_day_start() {
    display::clock(at)
  } else {
    day_and_time(ctx, at)
  }
}

/// What a server runs, `Orchester server 0.4.0, API 3`; `API 3` from a server that names no version.
pub fn runs(ctx: &Ctx, version: &str, api: u8) -> String {
  if version.is_empty() {
    ctx.t_args("cloud.server.api", [("api", api.to_string())]).to_string()
  } else {
    ctx
      .t_args(
        "cloud.server.runs",
        [("version", version.to_owned()), ("api", api.to_string())],
      )
      .to_string()
  }
}

/// `count` of a thing, with the key for one or for more.
pub fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> String {
  if count == 1 {
    ctx.t(one).to_string()
  } else {
    ctx.t_args(other, [("count", count.to_string())]).to_string()
  }
}
