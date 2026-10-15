//! The steps of *Connect a server* on their rail (`Steps` `YlIGu` in `zxe2P`): each a 22 dot (a check once done, its
//! number on `text_primary` while current, in a `border_strong` ring later) with a line down to the next, its title
//! (14/600) and its content. *Address* and *Compare the check code* are here; *Join* and *Your username there* are in
//! [`super::join`].

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::client::{PlainHttp, ServerAddress};
use orchester_workspace::sharing::{Reaching, ServerEntry};

use super::{ConnectPage, Found, Stage, join};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{OrchesterView, Route},
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterSpacing, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    toggle::{Toggle, ToggleKind},
  },
};

/// A step's dot (`l5QYK8`), and the check inside a done one.
const DOT: f32 = 22.0;
/// The address field's width in the first step (`ZJytn`).
const FIELD_WIDTH: f32 = 420.0;

/// Where a step stands.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
  Done,
  Current(u8),
  Later(u8),
}

pub(super) fn steps(ctx: &mut Ctx, page: &ConnectPage, entries: &[ServerEntry]) -> Element {
  let found = page.found();
  let stage = page.stage.get();
  let own = found
    .as_ref()
    .is_some_and(|found| matches!(found.reached.trust, Reaching::OwnCertificate(_)));
  let address_mark = if found.is_some() { Mark::Done } else { Mark::Current(1) };
  let compare_mark = match stage.clone() {
    Stage::Compare | Stage::Mismatch => Mark::Current(2),
    Stage::Join => Mark::Done,
    Stage::Address => Mark::Later(2),
  };
  let (join_number, name_number) = if own { (3, 4) } else { (2, 3) };
  let join_mark = if stage == Stage::Join {
    Mark::Current(join_number)
  } else {
    Mark::Later(join_number)
  };
  let address_title = ctx.t("sharing.connect.address.title").to_string();
  let address_content = address(ctx, page, (found.as_ref(), entries));
  let mut column = Column::new()
    .id("orchester-connect-steps")
    .width(Dimension::Pct(100.0))
    .child(step(
      ("orchester-connect-step-address", address_mark, &address_title),
      address_content,
      true,
    ));
  if own || found.is_none() {
    let content = match (&found, stage.clone()) {
      (Some(found), Stage::Compare | Stage::Mismatch) => compare(ctx, page, found),
      (Some(found), _) => compared_line(ctx, found),
      (None, _) => later(ctx, "sharing.connect.compare.later"),
    };
    let title = if matches!(compare_mark, Mark::Done) {
      ctx.t("sharing.connect.compare.done").to_string()
    } else {
      ctx.t("sharing.connect.compare.title").to_string()
    };
    column = column.child(step(
      ("orchester-connect-step-compare", compare_mark, &title),
      content,
      true,
    ));
  }
  let joining = found.as_ref().filter(|_| stage == Stage::Join);
  let join_title = ctx.t("sharing.connect.join.title").to_string();
  let join_content = match joining {
    Some(_) => join::join_step(ctx, page),
    None => later(ctx, "sharing.connect.join.later"),
  };
  column = column.child(step(
    ("orchester-connect-step-join", join_mark, &join_title),
    join_content,
    true,
  ));
  let name_title = ctx.t("sharing.connect.username.title").to_string();
  let name_mark = if stage == Stage::Join {
    Mark::Current(name_number)
  } else {
    Mark::Later(name_number)
  };
  let name_content = match joining {
    Some(found) => join::username_step(ctx, page, found),
    None => later_with(ctx, "sharing.connect.username.later", found.as_ref()),
  };
  column
    .child(step(
      ("orchester-connect-step-username", name_mark, &name_title),
      name_content,
      false,
    ))
    .into()
}

/// A step: its dot and line on the rail, its title, and `content` under the title.
fn step((id, mark, title): (&str, Mark, &str), content: Element, line: bool) -> Row {
  let later = matches!(mark, Mark::Later(_));
  let mut rail = Column::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(dot(mark));
  if line {
    rail = rail.child(
      Row::new()
        .flex(1.0)
        .width(Dimension::Px(0.0))
        .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border)),
    );
  }
  let title = Text::new(title)
    .id(format!("{id}-title"))
    .variant(OrchesterText::StepTitle.role())
    .color(if later {
      PaletteColor::TextMuted
    } else {
      PaletteColor::TextPrimary
    });
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(OrchesterSpacing::FormGap.role())
    .describe(
      "state",
      match mark {
        Mark::Done => "done",
        Mark::Current(_) => "current",
        Mark::Later(_) => "later",
      },
    )
    .child(rail)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .padding(Padding::new().bottom(OrchesterSpacing::FormTop.role()))
        .spacing(ExtraSpacing::Inline.role())
        .child(title)
        .child(content),
    )
}

fn dot(mark: Mark) -> Row {
  let base = Row::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Px(DOT))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Full.role());
  match mark {
    Mark::Done => {
      base
        .background(PaletteColor::SuccessMuted)
        .child(icon(Icon::Check, IconSize::Sm, PaletteColor::Success))
    }
    Mark::Current(number) => base.background(PaletteColor::TextPrimary).child(
      Text::new(&number.to_string())
        .variant(OrchesterText::StepNumber.role())
        .nowrap(),
    ),
    Mark::Later(number) => base
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .child(
        Text::new(&number.to_string())
          .variant(OrchesterText::StepNumber.role())
          .color(PaletteColor::TextMuted)
          .nowrap(),
      ),
  }
}

/// What a later step will ask, muted.
fn later(ctx: &Ctx, key: &str) -> Element {
  Text::new(&ctx.t(key))
    .variant(ExtraTypography::Lead.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}

fn later_with(ctx: &Ctx, key: &str, found: Option<&Found>) -> Element {
  let server = found.map_or_else(
    || ctx.t("sharing.connect.theServer").to_string(),
    |found| found.reached.info.name.clone(),
  );
  Text::new(&ctx.t_args(key, [("server", server)]))
    .variant(ExtraTypography::Lead.role())
    .color(PaletteColor::TextMuted)
    .width(Dimension::Pct(100.0))
    .into()
}

/// *Address*: the field and *Continue* with what answered there, or the server found with *Change*.
fn address(ctx: &mut Ctx, page: &ConnectPage, (found, entries): (Option<&Found>, &[ServerEntry])) -> Element {
  if let Some(found) = found {
    return server_found(ctx, page, (found, entries));
  }
  let state = page.reach.state().get();
  let pending = state.is_pending();
  let failed = state.error.clone().filter(|_| !pending);
  let typed = page.address.get();
  let parsed = ServerAddress::parse(&typed).ok();
  let needs_opt_in = parsed.as_ref().is_some_and(ServerAddress::needs_opt_in);
  let mut input = FieldInput::new(
    "orchester-connect-address",
    ctx.t("sharing.connect.address.label"),
    page.address.clone(),
  )
  .mono()
  .placeholder(ctx.t("sharing.connect.address.placeholder"));
  if failed.is_some() {
    input = input.invalid();
  }
  let allowed = page.allow_plain.get();
  let (reach, address, allow) = (page.reach.clone(), page.address.clone(), page.allow_plain.clone());
  let mut go = ActionButton::new(
    "orchester-connect-continue",
    ButtonKind::Primary,
    ctx.t("sharing.connect.continue"),
  )
  .icon(Icon::ArrowRight);
  go = if pending {
    go.loading()
  } else if needs_opt_in && !allowed {
    go.disabled()
  } else {
    go.on_click(move || reach.run((address.get(), allow.get())))
  };
  let mut column = Column::new()
    .width(Dimension::Px(FIELD_WIDTH))
    .spacing(SpacingSize::Md)
    .child(field(
      "orchester-connect-address-field",
      Some(&ctx.t("sharing.connect.address.label")),
      input.render(ctx),
      None,
    ));
  if let Some(reason) = &failed {
    column = column.child(field_error("orchester-connect-address-error", &reason.text(ctx)));
  }
  if needs_opt_in {
    column = column.child(plain_http(ctx, page, parsed.as_ref()));
  }
  column = column.child(Row::new().child(go));
  if let Some(found) = state
    .data
    .clone()
    .filter(|found| state.is_fulfilled() && found.reached.info.needs_bootstrap)
  {
    column = column.child(not_set_up(ctx, &found));
  }
  column
    .child(
      Text::new(&ctx.t("sharing.connect.address.hint"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// *This address isn't encrypted* with the explicit allow (`Plain HTTP · allowed only on purpose` in `ZJytn`).
fn plain_http(ctx: &Ctx, page: &ConnectPage, address: Option<&ServerAddress>) -> Element {
  let authority = address.map(ServerAddress::authority).unwrap_or_default();
  let allow = page.allow_plain.clone();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Notice::new(
        "orchester-connect-plain",
        NoticeTone::Warning,
        ctx.t("sharing.connect.plain.title"),
      )
      .icon(Icon::LockKeyhole)
      .body(ctx.t("sharing.connect.plain.body")),
    )
    .child(
      Toggle::new(
        "orchester-connect-allow-plain",
        ToggleKind::Checkbox,
        page.allow_plain.get(),
        ctx.t_args("sharing.connect.plain.allow", [("address", authority)]),
      )
      .on_change(move |on| allow.set(on)),
    )
    .into()
}

/// *This server has no admin yet* with its admin in the browser (`Server isn't set up yet` in `ZJytn`).
fn not_set_up(ctx: &mut Ctx, found: &Found) -> Element {
  let services = Services::of(ctx);
  let url = format!("{}/admin", found.address.as_str());
  let links = services.links;
  let label = ctx.t_args("sharing.connect.setup.open", [("host", found.address.authority())]);
  Notice::new(
    "orchester-connect-not-set-up",
    NoticeTone::Warning,
    ctx.t("sharing.connect.setup.title"),
  )
  .icon(Icon::Settings)
  .body(ctx.t("sharing.connect.setup.body"))
  .action(
    ActionButton::new("orchester-connect-open-setup", ButtonKind::Secondary, label)
      .icon(Icon::ExternalLink)
      .on_click(move || {
        if let Err(error) = links.open(&url) {
          tracing::warn!(%error, "the server admin could not be opened");
        }
      }),
  )
  .into()
}

/// The server found (`Server found` `avDuF`): its name, address and API, and *Change*.
fn server_found(ctx: &mut Ctx, page: &ConnectPage, (found, entries): (&Found, &[ServerEntry])) -> Element {
  let change = {
    let stage = page.stage.clone();
    ActionButton::new(
      "orchester-connect-change",
      ButtonKind::Ghost,
      ctx.t("sharing.connect.change"),
    )
    .on_click(move || stage.set(Stage::Address))
  };
  let line = ctx.t_args(
    "sharing.connect.found.line",
    [
      ("address", found.address.as_str().to_owned()),
      ("api", found.reached.info.api_version.to_string()),
    ],
  );
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Row::new()
        .id("orchester-connect-found")
        .width(Dimension::Pct(100.0))
        .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Inline.role()))
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Center)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Md)
        .child(icon(Icon::Server, IconSize::Md, PaletteColor::TextSecondary))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .spacing(ExtraSpacing::Hair.role())
            .child(
              Text::new(&found.reached.info.name)
                .id("orchester-connect-found-name")
                .variant(ExtraTypography::Strong.role()),
            )
            .child(Text::new(&line).variant(ExtraTypography::Note.role()).nowrap()),
        )
        .child(change),
    );
  match found.reached.trust {
    Reaching::System => {
      column = column.child(
        Notice::new(
          "orchester-connect-system-trust",
          NoticeTone::Success,
          ctx.t_args(
            "sharing.connect.system.title",
            [("server", found.reached.info.name.as_str())],
          ),
        )
        .icon(Icon::ShieldCheck)
        .body(ctx.t("sharing.connect.system.body")),
      );
    }
    Reaching::Plain(PlainHttp::Allowed) => {
      column = column.child(
        Text::new(&ctx.t_args(
          "sharing.connect.plain.allowed",
          [("address", found.address.authority())],
        ))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Warning),
      );
    }
    Reaching::Plain(PlainHttp::NotAllowed) | Reaching::OwnCertificate(_) => {}
  }
  column = column.with_children(already_connected(ctx, found, entries));
  column.into()
}

/// *You're already connected to Kontur team*, when this computer is signed in there.
fn already_connected(ctx: &mut Ctx, found: &Found, entries: &[ServerEntry]) -> Option<Element> {
  let entry = entries
    .iter()
    .find(|entry| entry.address == found.address && entry.in_use())?;
  let navigation = Navigation::of(ctx);
  let id = entry.stored.id;
  Some(
    Notice::new(
      "orchester-connect-already",
      NoticeTone::Info,
      ctx.t_args(
        "sharing.connect.already.title",
        [("server", entry.stored.name.as_str())],
      ),
    )
    .body(ctx.t("sharing.connect.already.body"))
    .action(
      ActionButton::new(
        "orchester-connect-open-entry",
        ButtonKind::Secondary,
        ctx.t("sharing.connect.already.open"),
      )
      .on_click(move || navigation.replace(Route::Orchester(OrchesterView::server(id)))),
    )
    .into(),
  )
}

/// *Compare the check code*: the code, *It matches* (a person only) and *It doesn't match*.
fn compare(ctx: &mut Ctx, page: &ConnectPage, found: &Found) -> Element {
  let Reaching::OwnCertificate(presented) = &found.reached.trust else {
    return Column::new().into();
  };
  let name = found.reached.info.name.as_str();
  if page.stage.get() == Stage::Mismatch {
    let back = {
      let stage = page.stage.clone();
      ActionButton::new(
        "orchester-connect-back",
        ButtonKind::Secondary,
        ctx.t("sharing.connect.compare.back"),
      )
      .on_click(move || stage.set(Stage::Address))
    };
    return Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(
        Notice::new(
          "orchester-connect-mismatch",
          NoticeTone::Danger,
          ctx.t("sharing.connect.compare.notConnected"),
        )
        .icon(Icon::ShieldAlert)
        .body(ctx.t("sharing.connect.compare.mismatchBody")),
      )
      .child(Row::new().child(back))
      .into();
  }
  let gate = ctx.use_context::<InputGate>();
  let (stage, join) = (page.stage.clone(), page.join.clone());
  let chosen = presented.clone();
  let matches = ActionButton::new(
    "orchester-connect-matches",
    ButtonKind::Primary,
    ctx.t("sharing.connect.compare.matches"),
  )
  .icon(Icon::Check)
  .on_click(move || {
    // Only a person compares the code with the server's console; synthetic input never confirms it.
    if input_may_decide(gate.as_ref(), Guarded::CheckCode) {
      join.confirm(chosen.clone().confirm());
      stage.set(Stage::Join);
    }
  });
  let differs = {
    let stage = page.stage.clone();
    ActionButton::new(
      "orchester-connect-differs",
      ButtonKind::Secondary,
      ctx.t("sharing.connect.compare.differs"),
    )
    .on_click(move || stage.set(Stage::Mismatch))
  };
  let compared = super::super::code::compared(
    "orchester-connect-check",
    &presented.check_code(),
    vec![
      Row::new().spacing(SpacingSize::Sm).child(matches).child(differs).into(),
      Text::new(&ctx.t_args("sharing.connect.compare.person", [("server", name)]))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
        .into(),
    ],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Text::new(&ctx.t_args("sharing.connect.compare.lead", [("server", name)]))
        .variant(ExtraTypography::Lead.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(compared)
    .into()
}

/// The check code once it matched (`Check code` line in `bIwte`).
fn compared_line(ctx: &Ctx, found: &Found) -> Element {
  let code = match &found.reached.trust {
    Reaching::OwnCertificate(presented) => presented.check_code(),
    _ => String::new(),
  };
  Text::new(&ctx.t_args("sharing.connect.compare.matched", [("code", code)]))
    .id("orchester-connect-check-matched")
    .variant(ExtraTypography::Note.role())
    .into()
}
