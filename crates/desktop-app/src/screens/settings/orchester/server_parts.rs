//! The top of a team server's page (`05c · Team server` `C6qnY`, its states in `ZUuMT`): the head — a server tile, the
//! server's name, its state and what it runs, *Server admin* and *…* with *Sign out…* — then what is wrong (a changed
//! certificate to compare `N1rNun`, an ended sign-in to join again `f5Psv`) and *Sign out of Kontur team on this
//! computer?* in place (`zNDt4`). The groups under them are in [`super::server_facts`].

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::sharing::{Connection, LinkedProject, ServerEntry};

use super::{
  Said,
  code::compared,
  list::state,
  parts::{hint, paragraph},
  server::{JoinedAgain, Presented, Props, ServerPanel, check_code},
  words::server_look,
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  screens::settings::frame,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::head_tile,
  },
};

/// The buttons only a person presses ([`super::PersonOnly`]).
const JOIN_AGAIN: &str = "orchester-server-join-again";
const SIGN_OUT: &str = "orchester-server-sign-out-confirm";
const CERTIFICATE_MATCHES: &str = "orchester-server-certificate-matches";

/// The head (`Head` `oPhLG`): the tile, the name, the state with what the server runs (`runs`), *Server admin* while
/// it is reachable, and *…*.
pub(super) fn head(ctx: &mut Ctx, panel: &ServerPanel, (entry, runs): (&ServerEntry, Option<String>)) -> Element {
  let separator = ctx.t("cloud.factSeparator");
  let line = Row::new()
    .id("orchester-server-header-facts")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(state(ctx, "orchester-server-header-state", &server_look(entry)))
    .child(
      Text::new(&separator)
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(&runs.unwrap_or_else(|| entry.address.authority()))
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  let reachable = entry
    .connection
    .is_some_and(|connection| connection != Connection::SignedOut);
  let mut actions: Vec<Element> = Vec::new();
  if reachable {
    actions.push(super::server_facts::open_admin(ctx, panel, entry, ButtonKind::Secondary).into());
  }
  if entry.in_use() {
    actions.push(more(ctx, panel));
  }
  let tile = head_tile(icon(Icon::Server, IconSize::Md, PaletteColor::TextSecondary));
  frame::head(
    "orchester-server-header",
    Some(tile.into()),
    (&entry.stored.name, Some(line.into())),
    actions,
  )
  .into()
}

/// *…* and its menu: *Sign out…*, which asks in place.
fn more(ctx: &mut Ctx, panel: &ServerPanel) -> Element {
  let (open, anchor) = panel.more.clone();
  let toggle = open.clone();
  let button = IconButton::new("orchester-server-more", Icon::Ellipsis, ctx.t("cloud.server.more"))
    .element(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
  let shown = open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let confirming = panel.confirming.clone();
    let item = MenuItem::new("orchester-server-sign-out", ctx.t("cloud.server.signOut"), move || {
      confirming.set(true)
    })
    .icon(Icon::LogOut);
    menu_at_end(
      anchor,
      open.clone(),
      menu_panel("orchester-server-more-menu").child(item.render(ctx, &open)),
    )
  });
  Row::new().child(button).with_children(menu).into()
}

/// What is wrong with the server, first on its page, and what came of an answer to it.
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

pub(super) fn project_names(linked: &[LinkedProject]) -> String {
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
    ButtonKind::DangerGhost,
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
  .opens(navigation.to(Route::Orchester(OrchesterView {
    server: Some(id),
    panel: OrchesterPanel::Connect,
  })));
  let mut notice = SettingNotice::new("orchester-server-signed-out-notice", NoticeTone::Warning)
    .title(ctx.t_args(
      if entry.in_use() {
        "cloud.server.signedOut.title"
      } else {
        "cloud.server.signedOut.byYouTitle"
      },
      [("server", entry.stored.name.as_str())],
    ))
    .body(ctx.t(if entry.in_use() {
      "cloud.server.signedOut.body"
    } else {
      "cloud.server.signedOut.byYouBody"
    }))
    .action(again)
    .action(with_code);
  if let Some(reason) = panel.person.reason(ctx, JOIN_AGAIN) {
    notice = notice.part(reason);
  }
  match state.data.clone().filter(|_| state.is_fulfilled()) {
    Some(JoinedAgain::Waiting) => {
      notice = notice.note(ctx.t("cloud.server.signedOut.waiting"));
    }
    Some(JoinedAgain::NotInvited) => {
      notice = notice.part(field_error(
        "orchester-server-join-not-invited",
        &ctx.t_args(
          "sharing.connect.notInvitedServer",
          [("server", entry.stored.name.as_str())],
        ),
      ));
    }
    Some(JoinedAgain::Joined) | None => {}
  }
  Column::new()
    .id("orchester-server-signed-out")
    .width(Dimension::Pct(100.0))
    .child(notice)
    .into()
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
  let notice = SettingNotice::new("orchester-server-sign-out-question", NoticeTone::Warning)
    .title(ctx.t_args("cloud.server.signOutQuestion.title", [("server", name)]))
    .part(paragraph("orchester-server-sign-out-affects", &affects).variant(ExtraTypography::Prose.role()))
    .part(
      paragraph(
        "orchester-server-sign-out-forgets",
        &ctx.t_args("cloud.server.signOutQuestion.forgets", [("server", name)]),
      )
      .variant(ExtraTypography::Prose.role()),
    )
    .action(confirm)
    .action(
      ActionButton::new(
        "orchester-server-sign-out-keep",
        ButtonKind::Ghost,
        ctx.t("cloud.server.signOutQuestion.keep"),
      )
      .on_click(move || keep.set(false)),
    );
  match panel.person.reason(ctx, SIGN_OUT) {
    Some(reason) => notice.part(reason).into(),
    None => notice.into(),
  }
}

/// The server admin's address, which opens in the browser.
pub(super) fn admin_url(entry: &ServerEntry) -> String {
  format!("{}/admin", entry.address.as_str())
}

/// Opens `url` in the browser, or says why it could not, under the page's head.
pub(super) fn opener(ctx: &mut Ctx, panel: &ServerPanel, url: String) -> impl Fn() + Send + Sync + 'static {
  let (links, opened) = (Services::of(ctx).links, panel.opened.clone());
  move || match links.open(&url) {
    Ok(()) => opened.set(None),
    Err(error) => {
      tracing::warn!(%error, "the server admin could not be opened");
      opened.set(Some(
        Said::new("cloud.server.admin.openFailed").with("url", url.clone()),
      ));
    }
  }
}
