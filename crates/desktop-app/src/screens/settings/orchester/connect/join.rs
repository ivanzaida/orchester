//! *Join* and *Your username there* (`bIwte`): with the Orchester account (the cloud names the server and its
//! certificate, and gives a token for it) or with an invite code (8 characters, read without regard to case or the
//! hyphen), a username and a name for someone new on the server; then what came of it, with the server's entry.

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_core::id::TeamServerId;
use orchester_workspace::sharing::{
  CodeRequest, Reaching, Trust,
  team_join::{JoinOutcome, JoinedServer},
};

use super::{ConnectPage, Found, Shown, join_error};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  services::Services,
  theme::OrchesterSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::Icon,
    option_card::OptionCard,
  },
};

/// *Join Kontur team*, which only a person presses (review 1, M1).
const JOIN: &str = "orchester-connect-join";

/// The fields' width in the username step (`bIwte`).
const NAME_FIELD_WIDTH: f32 = 336.0;

/// How the person joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Method {
  Account,
  Code,
}

impl DevtoolsInspectable for Method {}

/// The pin the person confirmed in the step before, kept for the join.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Confirmed(pub Option<Trust>);

impl DevtoolsInspectable for Confirmed {}

/// What joining came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
  Joined(JoinedServer),
  /// The server holds the join until another computer of the person's, or an admin, approves it.
  Waiting,
  /// The server has no place for the account (`With your Orchester account · nothing for you there`).
  NotInvited,
}

impl DevtoolsInspectable for Outcome {}

/// What the join sends.
#[derive(Clone)]
pub(super) struct Request {
  address: ServerAddress,
  /// The server's id at the cloud, as it names it at `GET /server`.
  linked: Option<TeamServerId>,
  trust: Trust,
  method: Method,
  code: String,
  username: String,
  name: String,
}

impl Request {
  /// The code, the username and the name as the server takes them: trimmed, a blank field as none.
  fn code(&self) -> CodeRequest {
    let filled = |text: &str| (!text.trim().is_empty()).then(|| text.trim().to_owned());
    CodeRequest {
      code: Credential::new(self.code.trim().to_owned()),
      username: filled(&self.username),
      display_name: filled(&self.name),
      proof: None,
    }
  }
}

/// The join's state on the page. Clones share it.
#[derive(Clone)]
pub(super) struct Join {
  confirmed: Signal<Confirmed>,
  method: Signal<Method>,
  code: Signal<String>,
  username: Signal<String>,
  name: Signal<String>,
  run: FutureAction<Request, Outcome, Shown>,
}

impl Join {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let team_join = services.team_join;
    let (username, name) = suggested(&services.cloud);
    Self {
      confirmed: ctx.signal(Confirmed(None)),
      method: ctx.signal(Method::Code),
      code: ctx.signal(String::new()),
      username: ctx.signal(username),
      name: ctx.signal(name),
      run: ctx.future_action(move |request: Request| {
        let team_join = team_join.clone();
        async move {
          let outcome = match request.method {
            Method::Account => team_join.join_server(&request.address, request.linked).await,
            Method::Code => {
              team_join
                .join_by_code(&request.address, request.trust, request.code())
                .await
            }
          };
          Ok(match outcome.map_err(|error| join_error(&error))? {
            JoinOutcome::Joined(joined) => Outcome::Joined(joined),
            JoinOutcome::Waiting(_) => Outcome::Waiting,
            JoinOutcome::NotInvited => Outcome::NotInvited,
          })
        }
      }),
    }
  }

  /// The person confirmed the server's check code: this computer trusts exactly that certificate there.
  pub(super) fn confirm(&self, trust: Trust) {
    self.confirmed.set(Confirmed(Some(trust)));
  }
}

/// The username and the name to start from: the signed-in account's (`ivan` and *Ivan Zaida* in `bIwte`), the
/// username taken from the email's local part with only the characters a server accepts.
fn suggested(cloud: &orchester_workspace::sharing::cloud::Cloud) -> (String, String) {
  let account = match cloud.account() {
    Ok(account) => account,
    Err(error) => {
      tracing::warn!(%error, "the cloud account could not be read to suggest a username");
      None
    }
  };
  account.map_or_else(Default::default, |account| {
    let local = account.email.split('@').next().unwrap_or_default();
    let username = local
      .chars()
      .filter(|char| char.is_ascii_alphanumeric() || "._-".contains(*char))
      .take(USERNAME_MAX)
      .collect::<String>()
      .to_ascii_lowercase();
    (username, account.display_name)
  })
}

/// The longest username a server accepts.
const USERNAME_MAX: usize = 64;

/// How this computer trusts what was found: the confirmed pin, the system's trust store, or plain HTTP as allowed.
fn trust_of(join: &Join, found: &Found) -> Option<Trust> {
  match found.reached.trust {
    Reaching::OwnCertificate(_) => join.confirmed.get().0,
    Reaching::System => Some(Trust::System(PlainHttp::NotAllowed)),
    Reaching::Plain(plain_http) => Some(Trust::System(plain_http)),
  }
}

/// *Join*: the two ways, and the invite code's field.
pub(super) fn join_step(ctx: &mut Ctx, page: &ConnectPage) -> Element {
  let join = &page.join;
  let account = Services::of(ctx).cloud.account().ok().flatten();
  let method = join.method.get();
  let account_description = account.as_ref().map_or_else(
    || ctx.t("sharing.connect.join.accountSignedOut").to_string(),
    |account| {
      ctx
        .t_args("sharing.connect.join.accountBody", [("email", account.email.as_str())])
        .to_string()
    },
  );
  let choose = |chosen: Method| {
    let method = join.method.clone();
    move || method.set(chosen)
  };
  let mut by_account = OptionCard::new(
    "orchester-connect-by-account",
    (Icon::Cloud, ctx.t("sharing.connect.join.account")),
    account_description,
    method == Method::Account,
    choose(Method::Account),
  );
  if account.is_none() {
    by_account = by_account.unavailable(ctx.t("sharing.connect.join.accountUnavailable"));
  }
  let by_code = OptionCard::new(
    "orchester-connect-by-code",
    (Icon::KeyRound, ctx.t("sharing.connect.join.code")),
    ctx.t("sharing.connect.join.codeBody"),
    method == Method::Code,
    choose(Method::Code),
  );
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Column::new().flex(1.0).child(by_account))
        .child(Column::new().flex(1.0).child(by_code)),
    );
  if method == Method::Code {
    let input = FieldInput::new(
      "orchester-connect-code",
      ctx.t("sharing.connect.code.label"),
      join.code.clone(),
    )
    .mono()
    .placeholder(ctx.t("sharing.connect.code.placeholder"))
    .width(Dimension::Px(NAME_FIELD_WIDTH))
    .render(ctx);
    column = column.child(field(
      "orchester-connect-code-field",
      Some(&ctx.t("sharing.connect.code.label")),
      input,
      Some(&ctx.t("sharing.connect.code.hint")),
    ));
  }
  column.into()
}

/// *Your username there*, *Join Kontur team* and what came of it.
pub(super) fn username_step(ctx: &mut Ctx, page: &ConnectPage, found: &Found) -> Element {
  let join = &page.join;
  let state = join.run.state().get();
  if let Some(outcome) = state.data.clone().filter(|_| state.is_fulfilled()) {
    return outcome_part(ctx, &outcome, found);
  }
  let server = found.reached.info.name.as_str();
  let mut column = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Md);
  if join.method.get() == Method::Code {
    let username = FieldInput::new(
      "orchester-connect-username",
      ctx.t("sharing.connect.username.label"),
      join.username.clone(),
    )
    .mono()
    .render(ctx);
    let name = FieldInput::new(
      "orchester-connect-name",
      ctx.t("sharing.connect.username.name"),
      join.name.clone(),
    )
    .render(ctx);
    column = column.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .child(Column::new().width(Dimension::Px(NAME_FIELD_WIDTH)).child(field(
          "orchester-connect-username-field",
          Some(&ctx.t("sharing.connect.username.label")),
          username,
          Some(&ctx.t("sharing.connect.username.hint")),
        )))
        .child(Column::new().width(Dimension::Px(NAME_FIELD_WIDTH)).child(field(
          "orchester-connect-name-field",
          Some(&ctx.t("sharing.connect.username.name")),
          name,
          Some(&ctx.t("sharing.connect.username.nameHint")),
        ))),
    );
  } else {
    column = column.child(
      lurq::components::Text::new(&ctx.t_args("sharing.connect.username.later", [("server", server)]))
        .variant(crate::theme::ExtraTypography::Lead.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
    column = column.child(field_error("orchester-connect-join-error", &reason.text(ctx)));
  }
  column
    .child(join_button(ctx, page, found))
    .with_children(page.person.reason(ctx, JOIN))
    .into()
}

/// *Join Kontur team*, with what was asked; unavailable until the check code is compared.
fn join_button(ctx: &Ctx, page: &ConnectPage, found: &Found) -> Row {
  let join = &page.join;
  let state = join.run.state().get();
  let trust = trust_of(join, found);
  let linked = found
    .reached
    .info
    .cloud
    .as_ref()
    .and_then(|cloud| cloud.server.parse().ok());
  let request = trust.map(|trust| Request {
    address: found.address.clone(),
    linked,
    trust,
    method: join.method.get(),
    code: join.code.get(),
    username: join.username.get(),
    name: join.name.get(),
  });
  let mut go = ActionButton::new(
    JOIN,
    ButtonKind::Primary,
    ctx.t_args(
      "sharing.connect.join.submit",
      [("server", found.reached.info.name.as_str())],
    ),
  )
  .icon(Icon::LogIn);
  go = match request {
    _ if state.is_pending() => go.loading(),
    Some(request) => {
      let (run, person) = (join.run.clone(), page.person.clone());
      go.on_click(move || {
        if person.allows(JOIN) {
          run.run(request.clone());
        }
      })
    }
    None => go.disabled(),
  };
  Row::new().child(go)
}

fn outcome_part(ctx: &mut Ctx, outcome: &Outcome, found: &Found) -> Element {
  let navigation = Navigation::of(ctx);
  let server = found.reached.info.name.as_str();
  match outcome {
    Outcome::Joined(JoinedServer { id, name, username }) => {
      let id = *id;
      // A code names no project: the server's entry lists the person's projects there, each with *Link a checkout…*,
      // which leads to the join review.
      let review = ActionButton::new(
        "orchester-connect-join-review",
        ButtonKind::Primary,
        ctx.t("sharing.connect.joined.review"),
      )
      .icon(Icon::ArrowRight)
      .opens(navigation.to(Route::Orchester(OrchesterView::server(id))));
      let entry = ActionButton::new(
        "orchester-connect-open-joined",
        ButtonKind::Secondary,
        ctx.t("sharing.connect.joined.open"),
      )
      .on_click(move || navigation.replace(Route::Orchester(OrchesterView::server(id))));
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(OrchesterSpacing::FormGap.role())
        .child(
          Callout::new(
            "orchester-connect-joined",
            CalloutTone::Success,
            ctx.t_args(
              "sharing.connect.joined.title",
              [("server", name.as_str()), ("username", username.as_str())],
            ),
          )
          .body(ctx.t("sharing.connect.joined.body")),
        )
        .child(
          Row::new()
            .spacing(SpacingSize::Sm)
            .align_items(Alignment::Start)
            .child(review)
            .child(entry),
        )
        .into()
    }
    Outcome::Waiting => Callout::new(
      "orchester-connect-waiting",
      CalloutTone::Warning,
      ctx.t("cloud.invites.approval.title"),
    )
    .icon(Icon::Hourglass)
    .body(ctx.t_args("cloud.invites.approval.body", [("server", server)]))
    .into(),
    Outcome::NotInvited => {
      let email = Services::of(ctx)
        .cloud
        .account()
        .ok()
        .flatten()
        .map(|account| account.email)
        .unwrap_or_default();
      Callout::new(
        "orchester-connect-not-invited",
        CalloutTone::Warning,
        ctx.t_args(
          "sharing.connect.notInvitedTitle",
          [("server", server), ("email", email.as_str())],
        ),
      )
      .icon(Icon::UserX)
      .body(ctx.t("sharing.connect.notInvitedBody"))
      .into()
    }
  }
}
