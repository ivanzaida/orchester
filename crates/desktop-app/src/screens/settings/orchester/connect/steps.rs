//! The steps of *Connect a server* as the rows of one box (`Steps` `A8uyj` in `05b · Connect a server` `MrVql`): a 1
//! `border` stroke, radius 10, a 1 `border` line between steps. A step is 14 over and under and 16 at its sides, 12
//! between its 22 dot (a check once done, its number on `text_primary` while current, in a `border_strong` ring later)
//! and its title (13/600) with its content 12 under it. *Address* and *Compare the check code* are here; *Join* and
//! *Your username there* are in [`super::join`].

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
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

/// A step's dot (`e2VVsP`), and the check inside a done one.
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
  // The check code is a step of its own until the address shows the server needs none.
  let (join_number, name_number) = if own || found.is_none() { (3, 4) } else { (2, 3) };
  let join_mark = if stage == Stage::Join {
    Mark::Current(join_number)
  } else {
    Mark::Later(join_number)
  };
  let address_title = ctx.t("sharing.connect.address.title").to_string();
  let address_content = address(ctx, page, (found.as_ref(), entries));
  // A server this computer is already signed in to ends the flow at its address: there is nothing to trust or join.
  let already = found.as_ref().is_some_and(|found| already_in(found, entries).is_some());
  let mut rows = vec![step(
    ("orchester-connect-step-address", address_mark, &address_title),
    address_content,
  )];
  if already {
    return boxed(rows);
  }
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
    rows.push(step(("orchester-connect-step-compare", compare_mark, &title), content));
  }
  let joining = found.as_ref().filter(|_| stage == Stage::Join);
  let join_title = ctx.t("sharing.connect.join.title").to_string();
  let join_content = match joining {
    Some(_) => join::join_step(ctx, page),
    None => later(ctx, "sharing.connect.join.later"),
  };
  rows.push(step(
    ("orchester-connect-step-join", join_mark, &join_title),
    join_content,
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
  rows.push(step(
    ("orchester-connect-step-username", name_mark, &name_title),
    name_content,
  ));
  boxed(rows)
}

/// The steps in their box, a line between each two.
fn boxed(rows: Vec<Row>) -> Element {
  let rows = rows.into_iter().enumerate().map(|(index, row)| {
    if index == 0 {
      row
    } else {
      row.border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    }
  });
  Column::new()
    .id("orchester-connect-steps")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(rows)
    .into()
}

/// A step: its dot, its title, and `content` under the title.
fn step((id, mark, title): (&str, Mark, &str), content: Element) -> Row {
  let later = matches!(mark, Mark::Later(_));
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
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .describe(
      "state",
      match mark {
        Mark::Done => "done",
        Mark::Current(_) => "current",
        Mark::Later(_) => "later",
      },
    )
    .child(dot(mark))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Md)
        .child(
          Row::new()
            .height(Dimension::Px(DOT))
            .align_items(Alignment::Center)
            .child(title),
        )
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
  // Once nothing answered, the button tries again (`Address · nothing answers` in `ZJytn`).
  let mut go = if failed.is_some() {
    ActionButton::new(
      "orchester-connect-continue",
      ButtonKind::Secondary,
      ctx.t("sharing.connect.tryAgain"),
    )
    .icon(Icon::RefreshCw)
  } else {
    ActionButton::new(
      "orchester-connect-continue",
      ButtonKind::Primary,
      ctx.t("sharing.connect.continue"),
    )
    .icon(Icon::ArrowRight)
  };
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
  if failed.is_none() {
    column = column.child(
      Text::new(&ctx.t("sharing.connect.address.hint"))
        .id("orchester-connect-address-hint")
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    );
  }
  column.into()
}

const ALLOW_PLAIN: &str = "orchester-connect-allow-plain";

/// *This address isn't encrypted* with the explicit allow (`Plain HTTP · allowed only on purpose` in `ZJytn`).
fn plain_http(ctx: &Ctx, page: &ConnectPage, address: Option<&ServerAddress>) -> Element {
  let authority = address.map(ServerAddress::authority).unwrap_or_default();
  let (allow, person) = (page.allow_plain.clone(), page.person.clone());
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Callout::new(
        "orchester-connect-plain",
        CalloutTone::Warning,
        ctx.t("sharing.connect.plain.title"),
      )
      .icon(Icon::LockKeyhole)
      .body(ctx.t("sharing.connect.plain.body")),
    )
    .child(
      Toggle::new(
        ALLOW_PLAIN,
        ToggleKind::Checkbox,
        page.allow_plain.get(),
        ctx.t_args("sharing.connect.plain.allow", [("address", authority)]),
      )
      .on_change(move |on| {
        // Sending to another computer in the clear is a person's choice.
        if person.allows(ALLOW_PLAIN) {
          allow.set(on);
        }
      }),
    )
    .with_children(page.person.reason(ctx, ALLOW_PLAIN))
    .into()
}

/// *This server has no admin yet* with its admin in the browser (`Server isn't set up yet` in `ZJytn`).
fn not_set_up(ctx: &mut Ctx, found: &Found) -> Element {
  let services = Services::of(ctx);
  let url = format!("{}/admin", found.address.as_str());
  let links = services.links;
  let label = ctx.t_args("sharing.connect.setup.open", [("host", found.address.authority())]);
  Callout::new(
    "orchester-connect-not-set-up",
    CalloutTone::Warning,
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

/// The server found with what its trust says, and *You're already connected* where this computer is signed in.
fn server_found(ctx: &mut Ctx, page: &ConnectPage, (found, entries): (&Found, &[ServerEntry])) -> Element {
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(found_row(ctx, page, found));
  match found.reached.trust {
    Reaching::System => {
      column = column.child(
        Callout::new(
          "orchester-connect-system-trust",
          CalloutTone::Success,
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
  if let Some(entry) = already_in(found, entries) {
    column = column.child(
      Callout::new(
        "orchester-connect-already",
        CalloutTone::Info,
        ctx.t_args(
          "sharing.connect.already.title",
          [("server", entry.stored.name.as_str())],
        ),
      )
      .body(ctx.t("sharing.connect.already.body")),
    );
  }
  column.into()
}

/// The server found (`Server found` `avDuF`): its name, address and API, and *Change*.
fn found_row(ctx: &Ctx, page: &ConnectPage, found: &Found) -> Row {
  let change = {
    let stage = page.stage.clone();
    ActionButton::new(
      "orchester-connect-change",
      ButtonKind::Ghost,
      ctx.t("sharing.connect.change"),
    )
    .on_click(move || stage.set(Stage::Address))
  };
  let runs =
    crate::screens::settings::orchester::runs(ctx, &found.reached.info.version, found.reached.info.api_version);
  let line = ctx.t_args(
    "sharing.connect.found.line",
    [("address", found.address.as_str().to_owned()), ("runs", runs)],
  );
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
    .child(change)
}

/// The server in the list at `found`'s address that this computer is signed in to (`Already connected` in `ZJytn`).
fn already_in<'a>(found: &Found, entries: &'a [ServerEntry]) -> Option<&'a ServerEntry> {
  entries
    .iter()
    .find(|entry| entry.address == found.address && entry.in_use())
}

/// *It matches* under the check code, which only a person presses.
const MATCHES: &str = "orchester-connect-matches";

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
        Callout::new(
          "orchester-connect-mismatch",
          CalloutTone::Danger,
          ctx.t("sharing.connect.compare.notConnected"),
        )
        .icon(Icon::ShieldAlert)
        .body(ctx.t("sharing.connect.compare.mismatchBody")),
      )
      .child(Row::new().child(back))
      .into();
  }
  let (stage, join, person) = (page.stage.clone(), page.join.clone(), page.person.clone());
  let chosen = presented.clone();
  let matches = ActionButton::new(MATCHES, ButtonKind::Primary, ctx.t("sharing.connect.compare.matches"))
    .icon(Icon::Check)
    .on_click(move || {
      // Only a person compares the code with the server's console; synthetic input never confirms it.
      if person.confirms_code(MATCHES) {
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
    std::iter::once(Row::new().spacing(SpacingSize::Sm).child(matches).child(differs).into())
      .chain(page.person.reason(ctx, MATCHES))
      .chain([
        Text::new(&ctx.t_args("sharing.connect.compare.person", [("server", name)]))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0))
          .into(),
      ])
      .collect(),
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
