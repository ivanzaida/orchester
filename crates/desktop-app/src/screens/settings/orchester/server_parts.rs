//! The parts of a team server's entry (`SFaAp`, `Aowlk`): what is wrong first (a changed certificate to compare, an
//! ended sign-in to join again), *Sign out of Kontur team on this computer?* in place, the facts, *Projects on this
//! computer*, *Your other computers* and *Server admin*.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_store::JoinedWith;
use orchester_workspace::sharing::{Connection, LinkedProject, ServerEntry, device};

use super::{
  Said,
  code::{Purpose, compared},
  parts::{hint, paragraph, section, section_title},
  server::{JoinedAgain, Presented, Props, ServerPanel, check_code},
  words::day,
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::Icon,
    inspector::{self, PropertyValue},
  },
};

/// The buttons only a person presses ([`super::PersonOnly`]).
const ADD_COMPUTER: &str = "orchester-server-add-computer";
const ADMIN_SIGN_IN: &str = "orchester-server-admin-sign-in";
const JOIN_AGAIN: &str = "orchester-server-join-again";
const SIGN_OUT: &str = "orchester-server-sign-out-confirm";
const CERTIFICATE_MATCHES: &str = "orchester-server-certificate-matches";

/// What is wrong with the server, first in the panel, and what came of an answer to it.
pub(super) fn problems(
  ctx: &mut Ctx,
  panel: &ServerPanel,
  (props, presented): (&Props, Option<&Result<Presented, Said>>),
) -> Vec<Element> {
  let entry = &props.entry;
  let mut parts = Vec::new();
  if entry.certificate_changed {
    parts.push(certificate_changed(ctx, panel, (props, presented)));
  } else if !entry.in_use() || entry.connection == Some(Connection::SignedOut) {
    parts.push(signed_out(ctx, panel, entry));
  }
  for (id, error) in [
    ("orchester-server-sign-out-error", panel.sign_out.state().get().error),
    ("orchester-server-pairing-error", panel.pairing.state().get().error),
    ("orchester-server-trust-error", panel.trust.state().get().error),
    ("orchester-server-join-error", panel.join_again.state().get().error),
  ] {
    if let Some(error) = error {
      parts.push(field_error(id, &error.text(ctx)).into());
    }
  }
  if let Some(opened) = panel.opened.get() {
    parts.push(field_error("orchester-server-open-error", &opened.text(ctx)).into());
  }
  parts
}

fn project_names(linked: &[LinkedProject]) -> String {
  linked
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect::<Vec<_>>()
    .join(", ")
}

/// *Kontur team presents a different certificate* with the new check code, which only a person confirms.
fn certificate_changed(
  ctx: &mut Ctx,
  panel: &ServerPanel,
  (props, presented): (&Props, Option<&Result<Presented, Said>>),
) -> Element {
  let entry = &props.entry;
  let projects = project_names(&props.linked);
  let body = if projects.is_empty() {
    ctx.t("cloud.server.certificate.bodyNoProjects")
  } else {
    ctx.t_args("cloud.server.certificate.body", [("projects", projects.as_str())])
  };
  let notice = Callout::new(
    "orchester-server-certificate-changed",
    CalloutTone::Danger,
    ctx.t_args(
      "cloud.server.certificate.title",
      [("server", entry.stored.name.as_str())],
    ),
  )
  .icon(Icon::ShieldAlert)
  .body(body);
  let mut column = Column::new()
    .id("orchester-server-certificate")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(notice);
  match presented {
    None => {
      column = column.child(hint(
        "orchester-server-certificate-reading",
        &ctx.t("cloud.server.certificate.reading"),
      ))
    }
    Some(Err(reason)) => column = column.child(field_error("orchester-server-certificate-unread", &reason.text(ctx))),
    Some(Ok(presented)) => {
      column = column
        .child(Text::new(&ctx.t("cloud.server.certificate.newCode")).variant(ExtraTypography::Property.role()))
        .child(new_code(ctx, panel, (entry, presented)));
    }
  }
  column.into()
}

/// The new check code under *… presents a different certificate*, with *It matches* (a person only) and *It doesn't
/// match*.
fn new_code(ctx: &mut Ctx, panel: &ServerPanel, (entry, presented): (&ServerEntry, &Presented)) -> Element {
  let was = entry
    .stored
    .certificate_pin
    .as_deref()
    .and_then(check_code)
    .unwrap_or_default();
  let (trust, chosen, person) = (panel.trust.clone(), presented.clone(), panel.person.clone());
  let matches = ActionButton::new(
    CERTIFICATE_MATCHES,
    ButtonKind::Primary,
    ctx.t("cloud.server.certificate.matches"),
  )
  .icon(Icon::Check)
  .on_click(move || {
    // A person compares the code with the server's console; synthetic input never confirms it.
    if person.confirms_code(CERTIFICATE_MATCHES) {
      trust.run(chosen.clone());
    }
  });
  let confirming = panel.confirming.clone();
  let differs = ActionButton::new(
    "orchester-server-certificate-differs",
    ButtonKind::Secondary,
    ctx.t("cloud.server.certificate.differs"),
  )
  .on_click(move || confirming.set(true));
  compared(
    "orchester-server-certificate-new",
    &presented.0.check_code(),
    vec![
      paragraph(
        "orchester-server-certificate-compare",
        &ctx.t_args("cloud.server.certificate.compare", [("was", was.as_str())]),
      )
      .into(),
      Row::new().spacing(SpacingSize::Sm).child(matches).child(differs).into(),
    ]
    .into_iter()
    .chain(panel.person.reason(ctx, CERTIFICATE_MATCHES))
    .chain([hint(
      "orchester-server-certificate-person",
      &ctx.t("cloud.server.certificate.person"),
    )
    .into()])
    .collect(),
  )
  .into()
}

/// *This computer's sign-in on Atlas research ended*, with *Join again with Orchester Cloud* and *With a code*.
fn signed_out(ctx: &mut Ctx, panel: &ServerPanel, entry: &ServerEntry) -> Element {
  let navigation = Navigation::of(ctx);
  let (join, person) = (panel.join_again.clone(), panel.person.clone());
  let state = panel.join_again.state().get();
  let mut again =
    ActionButton::new(JOIN_AGAIN, ButtonKind::Primary, ctx.t("cloud.server.signedOut.cloud")).icon(Icon::Cloud);
  again = if state.is_pending() {
    again.loading()
  } else {
    again.on_click(move || {
      if person.allows(JOIN_AGAIN) {
        join.run(());
      }
    })
  };
  let id = entry.stored.id;
  let with_code = ActionButton::new(
    "orchester-server-join-code",
    ButtonKind::Secondary,
    ctx.t("cloud.server.signedOut.code"),
  )
  .icon(Icon::KeyRound)
  .on_click(move || {
    navigation.go(Route::Orchester(OrchesterView {
      server: Some(id),
      panel: OrchesterPanel::Connect,
    }));
  });
  let mut column = Column::new()
    .id("orchester-server-signed-out")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Callout::new(
        "orchester-server-signed-out-notice",
        CalloutTone::Warning,
        ctx.t_args(
          if entry.in_use() {
            "cloud.server.signedOut.title"
          } else {
            "cloud.server.signedOut.byYouTitle"
          },
          [("server", entry.stored.name.as_str())],
        ),
      )
      .icon(Icon::LogOut)
      .body(ctx.t(if entry.in_use() {
        "cloud.server.signedOut.body"
      } else {
        "cloud.server.signedOut.byYouBody"
      })),
    )
    .child(Row::new().spacing(SpacingSize::Sm).child(again).child(with_code))
    .with_children(panel.person.reason(ctx, JOIN_AGAIN));
  match state.data.clone().filter(|_| state.is_fulfilled()) {
    Some(JoinedAgain::Waiting) => {
      column = column.child(hint(
        "orchester-server-join-waiting",
        &ctx.t("cloud.server.signedOut.waiting"),
      ));
    }
    Some(JoinedAgain::NotInvited) => {
      column = column.child(field_error(
        "orchester-server-join-not-invited",
        &ctx.t_args(
          "sharing.connect.notInvitedServer",
          [("server", entry.stored.name.as_str())],
        ),
      ));
    }
    Some(JoinedAgain::Joined) | None => {}
  }
  column.into()
}

/// *Sign out of Kontur team on this computer?* in place, naming the projects it affects.
pub(super) fn sign_out_question(ctx: &Ctx, panel: &ServerPanel, props: &Props) -> Element {
  let name = props.entry.stored.name.as_str();
  let projects = project_names(&props.linked);
  let affects = if projects.is_empty() {
    ctx.t("cloud.server.signOutQuestion.noProjects")
  } else {
    let key = if props.linked.len() == 1 {
      "cloud.server.signOutQuestion.project"
    } else {
      "cloud.server.signOutQuestion.projects"
    };
    ctx.t_args(key, [("projects", projects.as_str())])
  };
  let (sign_out, confirming, person) = (panel.sign_out.clone(), panel.confirming.clone(), panel.person.clone());
  let pending = panel.sign_out.state().get().is_pending();
  let confirm = ActionButton::new(
    SIGN_OUT,
    ButtonKind::Danger,
    ctx.t("cloud.server.signOutQuestion.confirm"),
  )
  .icon(Icon::LogOut);
  let confirm = if pending {
    confirm.loading()
  } else {
    confirm.on_click(move || {
      if person.allows(SIGN_OUT) {
        confirming.set(false);
        sign_out.run(());
      }
    })
  };
  let keep = panel.confirming.clone();
  Column::new()
    .id("orchester-server-sign-out-question")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Text::new(&ctx.t_args("cloud.server.signOutQuestion.title", [("server", name)]))
        .variant(ExtraTypography::Strong.role()),
    )
    .child(paragraph("orchester-server-sign-out-affects", &affects))
    .child(paragraph(
      "orchester-server-sign-out-forgets",
      &ctx.t_args("cloud.server.signOutQuestion.forgets", [("server", name)]),
    ))
    .child(
      Row::new().spacing(SpacingSize::Sm).child(confirm).child(
        ActionButton::new(
          "orchester-server-sign-out-keep",
          ButtonKind::Secondary,
          ctx.t("cloud.server.signOutQuestion.keep"),
        )
        .on_click(move || keep.set(false)),
      ),
    )
    .with_children(panel.person.reason(ctx, SIGN_OUT))
    .into()
}

/// Address, certificate, the account there, how and when this computer joined, and this computer.
/// How this computer trusts the server's certificate; a pinned one's check code reads in mono (`Checked · 3F7A-…` in
/// `SFaAp`).
fn certificate_fact(ctx: &Ctx, entry: &ServerEntry) -> (String, PropertyValue) {
  match entry.stored.certificate_pin.as_deref().and_then(check_code) {
    Some(code) => (
      ctx.t_args("cloud.server.facts.checked", [("code", code)]).to_string(),
      PropertyValue::Mono,
    ),
    None if entry.address.is_https() => (ctx.t("cloud.server.facts.system").to_string(), PropertyValue::Plain),
    None => (ctx.t("cloud.server.facts.plain").to_string(), PropertyValue::Plain),
  }
}

pub(super) fn facts(ctx: &Ctx, entry: &ServerEntry) -> Element {
  let stored = &entry.stored;
  let (certificate, certificate_look) = certificate_fact(ctx, entry);
  let you = stored.account.as_ref().map_or_else(
    || ctx.t("cloud.server.facts.nobody").to_string(),
    |account| {
      let role = if account.admin {
        ctx.t("cloud.server.facts.admin")
      } else {
        ctx.t("cloud.server.facts.member")
      };
      ctx
        .t_args(
          "cloud.server.facts.you",
          [
            ("name", account.display_name.as_str()),
            ("username", account.username.as_str()),
            ("role", &role),
          ],
        )
        .to_string()
    },
  );
  let joined_day = day(
    ctx,
    stored
      .account
      .as_ref()
      .map_or(stored.added_at, |account| account.signed_in_at),
  );
  let joined = match stored.joined_with {
    Some(JoinedWith::Cloud(_)) => ctx.t_args("cloud.server.facts.joinedCloud", [("day", joined_day)]),
    Some(JoinedWith::Code) => ctx.t_args("cloud.server.facts.joinedCode", [("day", joined_day)]),
    None => ctx.t_args("cloud.server.facts.joinedOn", [("day", joined_day)]),
  };
  let computer = ctx.t_args(
    if entry.in_use() {
      "cloud.server.facts.computerValue"
    } else {
      "cloud.server.facts.computerSignedOut"
    },
    [("computer", device::this_computer())],
  );
  Column::new()
    .id("orchester-server-facts")
    .width(Dimension::Pct(100.0))
    .child(inspector::property(
      "orchester-server-address",
      &ctx.t("cloud.server.facts.address"),
      entry.address.as_str(),
      PropertyValue::Mono,
    ))
    .child(inspector::property(
      "orchester-server-certificate",
      &ctx.t("cloud.server.facts.certificate"),
      &certificate,
      certificate_look,
    ))
    .child(inspector::property(
      "orchester-server-you",
      &ctx.t("cloud.server.facts.youThere"),
      &you,
      PropertyValue::Plain,
    ))
    .child(inspector::property(
      "orchester-server-joined",
      &ctx.t("cloud.server.facts.joined"),
      &joined,
      PropertyValue::Plain,
    ))
    .child(inspector::property(
      "orchester-server-computer",
      &ctx.t("cloud.server.facts.computer"),
      &computer,
      PropertyValue::Plain,
    ))
    .into()
}

/// *Your other computers* with *Add a computer*.
pub(super) fn computers(ctx: &Ctx, panel: &ServerPanel, entry: &ServerEntry) -> Element {
  let text = ctx.t_args("cloud.server.computers.body", [("server", entry.stored.name.as_str())]);
  let add = pairing_button(ctx, panel, (ADD_COMPUTER, Purpose::Computer));
  section(
    "orchester-server-computers",
    section_title(
      "orchester-server-computers-title",
      &ctx.t("cloud.server.computers.title"),
      None,
    ),
    std::iter::once(paragraph("orchester-server-computers-text", &text).into())
      .chain(std::iter::once(lurq::components::Row::new().child(add).into()))
      .chain(panel.person.reason(ctx, ADD_COMPUTER))
      .collect(),
  )
  .into()
}

fn pairing_button(ctx: &Ctx, panel: &ServerPanel, (id, purpose): (&'static str, Purpose)) -> ActionButton {
  let (label, glyph, kind) = match purpose {
    Purpose::Computer => (ctx.t("cloud.server.computers.add"), Icon::Laptop, ButtonKind::Secondary),
    Purpose::Admin => (ctx.t("cloud.server.admin.signIn"), Icon::KeyRound, ButtonKind::Ghost),
  };
  let (pairing, showing, person) = (panel.pairing.clone(), panel.showing.clone(), panel.person.clone());
  let button = ActionButton::new(id.to_owned(), kind, label).icon(glyph);
  if panel.pairing.state().get().is_pending() {
    button.disabled()
  } else {
    // A pairing code signs a computer or a browser in as this person: only a person asks for one (review 1, B2).
    button.on_click(move || {
      if person.allows(id) {
        showing.set(true);
        pairing.run(purpose);
      }
    })
  }
}

/// *Server admin*: *Open server admin* (a link) and *Sign in to the admin* (a pairing code).
pub(super) fn admin(ctx: &mut Ctx, panel: &ServerPanel, entry: &ServerEntry) -> Element {
  let admin_url = format!("{}/admin", entry.address.as_str());
  let admin = entry.stored.account.as_ref().is_some_and(|account| account.admin);
  let text = if admin {
    ctx.t_args(
      "cloud.server.admin.bodyAdmin",
      [("server", entry.stored.name.as_str()), ("url", admin_url.as_str())],
    )
  } else {
    ctx.t_args(
      "cloud.server.admin.bodyMember",
      [("server", entry.stored.name.as_str()), ("url", admin_url.as_str())],
    )
  };
  let services = Services::of(ctx);
  let (links, opened) = (services.links, panel.opened.clone());
  let url = admin_url;
  let open = ActionButton::new(
    "orchester-server-open-admin",
    ButtonKind::Secondary,
    ctx.t("cloud.server.admin.open"),
  )
  .icon(Icon::ExternalLink)
  .on_click(move || match links.open(&url) {
    Ok(()) => opened.set(None),
    Err(error) => {
      tracing::warn!(%error, "the server admin could not be opened");
      opened.set(Some(
        Said::new("cloud.server.admin.openFailed").with("url", url.clone()),
      ));
    }
  });
  let sign_in = pairing_button(ctx, panel, (ADMIN_SIGN_IN, Purpose::Admin));
  section(
    "orchester-server-admin",
    section_title("orchester-server-admin-title", &ctx.t("cloud.server.admin.title"), None),
    std::iter::once(paragraph("orchester-server-admin-text", &text).into())
      .chain(std::iter::once(
        lurq::components::Row::new()
          .spacing(SpacingSize::Sm)
          .child(open)
          .child(sign_in)
          .into(),
      ))
      .chain(panel.person.reason(ctx, ADMIN_SIGN_IN))
      .chain(std::iter::once(
        hint("orchester-server-admin-hint", &ctx.t("cloud.server.admin.hint")).into(),
      ))
      .collect(),
  )
  .into()
}

/// Where this computer's token for the server is kept.
pub(super) fn token_note(ctx: &Ctx, entry: &ServerEntry) -> Element {
  let mac = cfg!(target_os = "macos");
  let key = match (entry.in_use(), entry.connection) {
    // Signed out from here: the token was deleted with the sign-in.
    (false, _) if mac => "cloud.server.tokenNote.noneMacos",
    (false, _) => "cloud.server.tokenNote.noneWindows",
    (true, Some(Connection::SignedOut)) => "cloud.server.tokenNote.refused",
    (true, _) if mac => "cloud.server.tokenNote.macos",
    (true, _) => "cloud.server.tokenNote.windows",
  };
  inspector::note(
    "orchester-server-token-note",
    Icon::KeyRound,
    &ctx.t_args(key, [("server", entry.stored.name.as_str())]),
  )
  .into()
}
