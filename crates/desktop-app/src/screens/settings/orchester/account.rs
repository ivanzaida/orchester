//! The Orchester Cloud account at the top of the tab (`Account card` `f3tZy` in `05 · Orchester Cloud`): the person's
//! initials, name and two-factor authentication, their email and the cloud's host, and *Sign out*, which asks in place
//! under the card (`xQQbr`), and answering it is a person's. Then *This computer* (`W29jBR`): which device this is,
//! when it signed in, and the other devices, which this app doesn't show yet.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::{cloud::CloudInUse, device};

use super::{cloud::CloudPanel, words::day_and_time};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, SettingsText},
  ui::{
    avatar::initials,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::{SettingGroup, SettingRow, group_note, marked_words, value},
  },
};

/// The avatar's side (`Avatar` `voo5f`).
const AVATAR: f32 = 36.0;

const SIGN_OUT: &str = "orchester-cloud-sign-out";
pub(super) const SIGN_OUT_CONFIRM: &str = "orchester-cloud-sign-out-confirm";

/// The card: the avatar, the name with the two-factor line `after` it, the email and the host, and *Sign out*.
pub(super) fn card(
  ctx: &Ctx,
  panel: &CloudPanel,
  (account, address): (&StoredCloudAccount, &CloudInUse),
  after: Vec<Element>,
) -> Element {
  let avatar = Row::new()
    .id("orchester-cloud-avatar")
    .width(Dimension::Px(AVATAR))
    .height(Dimension::Px(AVATAR))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Full.role())
    .child(Text::new(&initials(&account.display_name)).variant(SettingsText::CardInitials.role()));
  let email = ctx.t_args(
    "cloud.account.emailAt",
    [
      ("email", account.email.as_str()),
      ("host", &address.address.web().authority()),
    ],
  );
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&account.display_name)
            .id("orchester-cloud-header-title")
            .variant(SettingsText::CardName.role())
            .nowrap(),
        )
        .with_children(after),
    )
    .child(
      Text::new(&email)
        .id("orchester-cloud-header-facts")
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0)),
    );
  let asking = panel.asking.clone();
  let sign_out = ActionButton::new(SIGN_OUT, ButtonKind::Secondary, ctx.t("cloud.account.signOut"))
    .icon(Icon::LogOut)
    .held(panel.asking.get())
    .on_click(move || asking.set(true));
  Row::new()
    .id("orchester-cloud-header")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .child(avatar)
    .child(text)
    .child(sign_out)
    .into()
}

/// *Sign out of Orchester Cloud on this computer?* in place (`xQQbr`), with what waits and what keeps working.
pub(super) fn sign_out_question(ctx: &Ctx, panel: &CloudPanel) -> Element {
  let (action, person) = (panel.sign_out.clone(), panel.person.clone());
  let confirm = ActionButton::new(
    SIGN_OUT_CONFIRM,
    ButtonKind::Danger,
    ctx.t("cloud.account.signOutConfirm.confirm"),
  )
  .icon(Icon::LogOut);
  let confirm = if panel.sign_out.state().get().is_pending() {
    confirm.loading()
  } else {
    confirm.on_click(move || {
      if person.allows(SIGN_OUT_CONFIRM) {
        action.run(());
      }
    })
  };
  let asking = panel.asking.clone();
  let stay = ActionButton::new(
    "orchester-cloud-sign-out-stay",
    ButtonKind::Ghost,
    ctx.t("cloud.account.signOutConfirm.stay"),
  )
  .on_click(move || asking.set(false));
  let notice = SettingNotice::new("orchester-cloud-sign-out-question", NoticeTone::Warning)
    .title(ctx.t("cloud.account.signOutConfirm.question"))
    .body(ctx.t("cloud.account.signOutConfirm.body"))
    .action(confirm)
    .action(stay);
  match panel.person.reason(ctx, SIGN_OUT_CONFIRM) {
    Some(reason) => notice.part(reason).into(),
    None => notice.into(),
  }
}

/// *This computer*: the device, when it signed in, and the other devices, not in this app yet; what signing out does
/// under it.
pub(super) fn this_computer(ctx: &mut Ctx, account: &StoredCloudAccount) -> Element {
  let device = ctx.t_args("cloud.account.deviceValue", [("device", device::this_computer())]);
  let signed_in = day_and_time(ctx, account.signed_in_at);
  let rows = vec![
    SettingRow::new("orchester-cloud-device", ctx.t("cloud.account.device"))
      .control(value("orchester-cloud-device-value", &device))
      .render(ctx),
    SettingRow::new("orchester-cloud-signed-in", ctx.t("cloud.account.signedInAt"))
      .hint(ctx.t("cloud.account.signedInHint"))
      .control(value("orchester-cloud-signed-in-value", &signed_in))
      .render(ctx),
    SettingRow::new("orchester-cloud-devices", ctx.t("cloud.account.otherDevices"))
      .hint(ctx.t("cloud.account.otherDevicesHint"))
      .info(ctx.t("cloud.account.tabs.devicesNotBuilt"))
      .control(marked_words(
        "orchester-cloud-devices-unavailable",
        Icon::Lock,
        &ctx.t("cloud.account.notInAppYet"),
      ))
      .render(ctx),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(SettingGroup::new("orchester-cloud-computer", ctx.t("cloud.account.thisComputer")).rows(rows))
    .child(group_note(
      "orchester-cloud-signing-out-text",
      &ctx.t("cloud.account.signingOutText"),
    ))
    .into()
}

/// The note at the column's end: where this platform keeps the cloud token.
pub(super) fn token_note(ctx: &Ctx) -> Text {
  group_note("orchester-cloud-token-note", &ctx.t(super::cloud::token_note()))
}
