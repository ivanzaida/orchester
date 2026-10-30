//! Joining with a code (`With a code` in `B914L`; `docs/design/startup.md` decision 4): the team server's address and
//! the code as typed, and the join they make. What answers at the address is asked first, sending nothing
//! ([`TeamJoin::reach`]); a server whose certificate the system trusts, or plain HTTP to this computer, is joined at
//! once with the code ([`TeamJoin::join_by_code`]). A server with its own certificate, plain HTTP to another computer
//! or a server not set up yet needs the person's check, which *Connect a server* in Settings › Orchester asks for.
//! Joining is a person's: synthetic input is refused, and the column says so. The code is held outside the
//! component's context, so DevTools never records it, and never reaches an error or a log.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::{ElementRef, Signal},
};
use orchester_api::{
  Credential,
  client::{ClientError, PlainHttp, ServerAddress},
};
use orchester_workspace::sharing::{
  CodeRequest, Reaching, SharingError, Trust,
  team_join::{JoinError, JoinRefusal, TeamJoin},
};

use super::outcome::Outcome;
use crate::{
  screens::settings::orchester::{
    PersonOnly,
    connect::{Shown, reach_error, suggested},
    shown_address,
  },
  services::Services,
};

/// *Join with the code*, which only a person presses.
pub(super) const SUBMIT: &str = "join-team-code-submit";

/// Why a code did not join, as the code column says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum CodeRefusal {
  /// What was typed is no server address.
  AddressInvalid,
  /// The server answers alike for a wrong, a used and an expired code (`aWsVx`).
  WrongCode,
  /// Nothing answered at the address (`oOn6j`).
  Unreachable {
    host: String,
  },
  /// The server needs the person's check first, in *Connect a server*.
  NeedsSettings {
    server: String,
  },
  Other(Shown),
}

impl DevtoolsInspectable for CodeRefusal {}

/// The name and the username to start from on a server where the person is new, as Settings suggests them.
#[derive(Clone)]
struct Suggested {
  username: String,
  name: String,
}

/// The code column's state. Clones share it.
#[derive(Clone)]
pub(super) struct CodeJoin {
  pub(super) address: Signal<String>,
  /// Made outside the component's context, so DevTools never records it.
  pub(super) code: Signal<String>,
  pub(super) run: FutureAction<(String, String), Outcome, CodeRefusal>,
  pub(super) person: PersonOnly,
  pub(super) address_input: ElementRef,
  /// Whether the address took the focus once.
  pub(super) focused: Signal<bool>,
}

impl CodeJoin {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let team_join = services.team_join;
    let (username, name) = suggested(&services.cloud);
    let suggestion = Suggested { username, name };
    Self {
      address: ctx.signal(String::new()),
      code: Signal::new(String::new()),
      run: ctx.future_action(move |typed: (String, String)| {
        let (team_join, suggestion) = (team_join.clone(), suggestion.clone());
        async move { join(&team_join, &suggestion, typed).await }
      }),
      person: PersonOnly::new(ctx),
      address_input: ElementRef::new(),
      focused: ctx.signal(false),
    }
  }

  /// Joins with what was typed, for *Join with the code*, *Try again* and Enter alike: nothing while a join is under
  /// way; synthetic input is refused, and the column says so under the button.
  pub(super) fn submitter(&self) -> impl Fn() + Send + Sync + 'static {
    let (run, address, code, person) = (
      self.run.clone(),
      self.address.clone(),
      self.code.clone(),
      self.person.clone(),
    );
    move || {
      if run.state().get_untracked().is_pending() || !person.allows(SUBMIT) {
        return;
      }
      run.run_if_idle((address.get_untracked(), code.get_untracked()));
    }
  }

  pub(super) fn pending(&self) -> bool {
    self.run.state().get().is_pending()
  }

  /// The refusal of the last join, once it ended.
  pub(super) fn refusal(&self) -> Option<CodeRefusal> {
    let state = self.run.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }

  /// What the last join came to, once it ended.
  pub(super) fn outcome(&self) -> Option<Outcome> {
    let state = self.run.state().get();
    let fulfilled = state.is_fulfilled();
    state.data.filter(|_| fulfilled)
  }
}

/// Joins the server at the typed address with the typed code.
async fn join(
  team_join: &TeamJoin,
  suggestion: &Suggested,
  (address, code): (String, String),
) -> Result<Outcome, CodeRefusal> {
  let address = ServerAddress::parse(address.trim()).map_err(|_| CodeRefusal::AddressInvalid)?;
  let reached = team_join
    .reach(&address, PlainHttp::NotAllowed)
    .await
    .map_err(|error| refused(&error, &address))?;
  let server = reached.info.name.clone();
  let trust = match reached.trust {
    _ if reached.info.needs_bootstrap => return Err(CodeRefusal::NeedsSettings { server }),
    Reaching::OwnCertificate(_) => return Err(CodeRefusal::NeedsSettings { server }),
    Reaching::System => Trust::System(PlainHttp::NotAllowed),
    Reaching::Plain(plain_http) => Trust::System(plain_http),
  };
  let filled = |text: &str| (!text.trim().is_empty()).then(|| text.trim().to_owned());
  let request = CodeRequest {
    code: Credential::new(code.trim().to_owned()),
    username: filled(&suggestion.username),
    display_name: filled(&suggestion.name),
    proof: None,
  };
  let outcome = team_join
    .join_by_code(&address, trust, request)
    .await
    .map_err(|error| refused(&error, &address))?;
  Ok(Outcome::of(outcome, &server))
}

/// `error` of a join at `address` in the column's words.
fn refused(error: &JoinError, address: &ServerAddress) -> CodeRefusal {
  match error {
    JoinError::Refused(JoinRefusal::WrongCode) => CodeRefusal::WrongCode,
    JoinError::Refused(JoinRefusal::Unreachable { address }) => CodeRefusal::Unreachable {
      host: shown_address(address.as_str()).to_owned(),
    },
    JoinError::Server(SharingError::Client(ClientError::PlainHttpRefused)) => CodeRefusal::NeedsSettings {
      server: shown_address(address.as_str()).to_owned(),
    },
    error => CodeRefusal::Other(reach_error(error, address)),
  }
}
