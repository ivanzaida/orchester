//! Orchester Cloud while it and the team servers are read (`16 · App settings · Orchester Cloud · loading` `Bq5lB`): the
//! account card keeps its avatar frame and Sign out; the This computer, Invites, Team servers and Advanced headings,
//! every row label and fixed help line, Connect a server and Change stay; the initials, name, two-factor state, e-mail,
//! device, sign-in time, invites, each server's line and state and the cloud address are bones, the lower groups
//! fading. *Reading Orchester Cloud and your team servers…* (`cloud.reading`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, SettingsSpacing, SettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    setting_rows::{SettingGroup, SettingRow, marked_words, opens_mark, row_tile},
    skeleton::{self, Skeleton, bone, fading},
  },
};

/// The avatar's side (`Avatar` `voo5f`).
const AVATAR: f32 = 36.0;

/// The tab's column while it is read; its button pressed opens nothing until the account is known.
pub(super) fn column(ctx: &mut Ctx) -> Element {
  let groups = vec![
    computer(ctx),
    Column::new().width(Dimension::Pct(100.0)).child(
      SettingGroup::new("orchester-reading-invites", ctx.t("cloud.invites.group")).row(
        SettingRow::new("orchester-reading-invite", "")
          .label_reading(82.0)
          .hint_reading(442.0)
          .lead(row_tile(icon(Icon::Mail, IconSize::Md, PaletteColor::TextSecondary)))
          .render(ctx),
      ),
    ),
    Column::new().width(Dimension::Pct(100.0)).child(servers(ctx)),
    Column::new().width(Dimension::Pct(100.0)).child(advanced(ctx)),
  ];
  let pane = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SettingsSpacing::Page.role())
    .child(card(ctx))
    .with_children(fading(groups));
  Skeleton::new("orchester-loading", ctx.t("cloud.reading")).show(ctx, pane)
}

/// The account card: its avatar frame with the initials' bone, the name, two-factor and e-mail bones, and Sign out,
/// which waits for the account.
fn card(ctx: &Ctx) -> Row {
  let avatar = Row::new()
    .width(Dimension::Px(AVATAR))
    .height(Dimension::Px(AVATAR))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Full.role())
    .child(bone(14.0, 8.0));
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(skeleton::line(ctx, SettingsText::CardName.role(), 86.0))
        .child(skeleton::status(ctx, 66.0)),
    )
    .child(skeleton::line(ctx, TypographyStyle::Description, 210.0));
  let sign_out = ActionButton::new(
    "orchester-reading-sign-out",
    ButtonKind::Secondary,
    ctx.t("cloud.account.signOut"),
  )
  .icon(Icon::LogOut)
  .disabled();
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(avatar)
    .child(text)
    .child(sign_out)
}

/// This computer: the device and when it signed in are bones; other devices say what they always say.
fn computer(ctx: &mut Ctx) -> Column {
  let rows = vec![
    SettingRow::new("orchester-reading-device", ctx.t("cloud.account.device"))
      .control(skeleton::line(ctx, ExtraTypography::Lead.role(), 176.0))
      .render(ctx),
    SettingRow::new("orchester-reading-signed-in", ctx.t("cloud.account.signedInAt"))
      .hint(ctx.t("cloud.account.signedInHint"))
      .control(skeleton::line(ctx, ExtraTypography::Lead.role(), 76.0))
      .render(ctx),
    SettingRow::new("orchester-reading-devices", ctx.t("cloud.account.otherDevices"))
      .hint(ctx.t("cloud.account.otherDevicesHint"))
      .control(marked_words(
        "orchester-reading-devices-unavailable",
        Icon::Lock,
        &ctx.t("cloud.account.notInAppYet"),
      ))
      .render(ctx),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(SettingGroup::new("orchester-reading-computer", ctx.t("cloud.account.thisComputer")).rows(rows))
}

/// Team servers: Connect a server real, a server's tile, name, line and state bones.
fn servers(ctx: &mut Ctx) -> SettingGroup {
  let connect = ActionButton::new(
    "orchester-reading-connect",
    ButtonKind::Ghost,
    ctx.t("settings.connectServer"),
  )
  .icon(Icon::Plus)
  .small()
  .disabled();
  let end = Row::new()
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(skeleton::status(ctx, 46.0))
    .child(opens_mark());
  let row = SettingRow::new("orchester-reading-server", "")
    .label_reading(120.0)
    .hint_reading(210.0)
    .lead(row_tile(bone(14.0, 14.0)))
    .control(end)
    .render(ctx);
  SettingGroup::new("orchester-reading-servers", ctx.t("cloud.list.teamServers"))
    .end(connect)
    .row(row)
}

/// Advanced: the cloud address a bone, Change real.
fn advanced(ctx: &mut Ctx) -> SettingGroup {
  let change = ActionButton::new("orchester-reading-change", ButtonKind::Ghost, ctx.t("cloud.advanced.change"))
    .icon(Icon::Pencil)
    .small()
    .disabled();
  let control = Row::new()
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(skeleton::line(ctx, ExtraTypography::PropertyMono.role(), 100.0))
    .child(change);
  let row = SettingRow::new("orchester-reading-address", ctx.t("cloud.advanced.row"))
    .hint(ctx.t("cloud.advanced.rowHint"))
    .control(control)
    .render(ctx);
  SettingGroup::new("orchester-reading-advanced", ctx.t("cloud.advanced.group")).row(row)
}
