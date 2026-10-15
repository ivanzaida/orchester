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
  node::{Element, dimension::Dimension},
};
use orchester_api::{
  Credential,
  client::{PlainHttp, ServerAddress},
};
use orchester_core::id::ServerId;
use orchester_workspace::sharing::{
  CloudJoined, CodeRequest, Reaching, Trust,
  cloud::{CloudError, ServerJoin},
};

use super::{ConnectPage, Found};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  screens::settings::orchester::{
    Said,
    words::{cloud_error, sharing_error},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::Icon,
    notice::{Notice, NoticeTone},
    option_card::OptionCard,
  },
};

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
  Joined {
    server: ServerId,
    name: String,
    username: String,
  },
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
  trust: Trust,
  method: Method,
  code: String,
  username: String,
  name: String,
}

/// The join's state on the page. Clones share it.
#[derive(Clone)]
pub(super) struct Join {
  confirmed: Signal<Confirmed>,
  method: Signal<Method>,
  code: Signal<String>,
  username: Signal<String>,
  name: Signal<String>,
  run: FutureAction<Request, Outcome, Said>,
}

impl Join {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (servers, cloud) = (services.servers, services.cloud);
    Self {
      confirmed: ctx.signal(Confirmed(None)),
      method: ctx.signal(Method::Code),
      code: ctx.signal(String::new()),
      username: ctx.signal(String::new()),
      name: ctx.signal(String::new()),
      run: ctx.future_action(move |request: Request| {
        let (servers, cloud) = (servers.clone(), cloud.clone());
        async move {
          match request.method {
            Method::Account => match cloud.join_server(&request.address).await {
              Ok(ServerJoin::Joined(CloudJoined::Joined(server))) => joined(&servers, server).await,
              Ok(ServerJoin::Joined(CloudJoined::Waiting(_))) => Ok(Outcome::Waiting),
              Ok(ServerJoin::NotInvited) => Ok(Outcome::NotInvited),
              Err(CloudError::NoServer) => Ok(Outcome::NotInvited),
              Err(error) => Err(cloud_error(&error)),
            },
            Method::Code => {
              let filled = |text: &str| (!text.trim().is_empty()).then(|| text.trim().to_owned());
              let code = CodeRequest {
                code: Credential::new(request.code.trim().to_owned()),
                username: filled(&request.username),
                display_name: filled(&request.name),
              };
              let server = servers
                .join_with_code(&request.address, request.trust, code)
                .await
                .map_err(|error| sharing_error(&error))?;
              joined(&servers, server).await
            }
          }
        }
      }),
    }
  }

  /// The person confirmed the server's check code: this computer trusts exactly that certificate there.
  pub(super) fn confirm(&self, trust: Trust) {
    self.confirmed.set(Confirmed(Some(trust)));
  }
}

/// The server joined, with the name and the username it has there now.
async fn joined(servers: &orchester_workspace::sharing::Servers, server: ServerId) -> Result<Outcome, Said> {
  let servers = servers.clone();
  let entry = orchester_workspace::background::run(move || servers.entries())
    .await
    .map_err(|_| Said::new("cloud.errors.local"))?
    .map_err(|error| sharing_error(&error))?
    .into_iter()
    .find(|entry| entry.stored.id == server)
    .ok_or_else(|| Said::new("cloud.errors.local"))?;
  Ok(Outcome::Joined {
    server,
    name: entry.stored.name,
    username: entry.stored.account.map(|account| account.username).unwrap_or_default(),
  })
}

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
  let trust = trust_of(join, found);
  let request = trust.map(|trust| Request {
    address: found.address.clone(),
    trust,
    method: join.method.get(),
    code: join.code.get(),
    username: join.username.get(),
    name: join.name.get(),
  });
  let mut go = ActionButton::new(
    "orchester-connect-join",
    ButtonKind::Primary,
    ctx.t_args("sharing.connect.join.submit", [("server", server)]),
  )
  .icon(Icon::LogIn);
  go = match request {
    _ if state.is_pending() => go.loading(),
    Some(request) => {
      let run = join.run.clone();
      go.on_click(move || run.run(request.clone()))
    }
    None => go.disabled(),
  };
  column.child(Row::new().child(go)).into()
}

fn outcome_part(ctx: &mut Ctx, outcome: &Outcome, found: &Found) -> Element {
  let navigation = Navigation::of(ctx);
  let server = found.reached.info.name.as_str();
  match outcome {
    Outcome::Joined {
      server: id,
      name,
      username,
    } => {
      let id = *id;
      Notice::new(
        "orchester-connect-joined",
        NoticeTone::Success,
        ctx.t_args(
          "sharing.connect.joined.title",
          [("server", name.as_str()), ("username", username.as_str())],
        ),
      )
      .body(ctx.t("sharing.connect.joined.body"))
      .action(
        ActionButton::new(
          "orchester-connect-open-joined",
          ButtonKind::Secondary,
          ctx.t("sharing.connect.joined.open"),
        )
        .icon(Icon::ArrowRight)
        .on_click(move || navigation.replace(Route::Orchester(OrchesterView::server(id)))),
      )
      .into()
    }
    Outcome::Waiting => Notice::new(
      "orchester-connect-waiting",
      NoticeTone::Warning,
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
      Notice::new(
        "orchester-connect-not-invited",
        NoticeTone::Warning,
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
