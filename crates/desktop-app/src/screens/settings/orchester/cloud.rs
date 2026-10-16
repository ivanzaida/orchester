//! The Orchester Cloud account (`Account details` `S9wde` in `Cu20b`): the header with *Sign out*; the tabs *Account*,
//! *Invites* and *Devices* (not built in this app yet, and says so); the account's facts, the waiting invites, what
//! signing out does and the advanced cloud address. *Sign out* asks in place under *Signing out* (`In-place
//! confirmation` `FeM5F` in `p2VeL`); *Sign out and lock* is a person's, and the sign-in gate takes the window after
//! it. Signed out, the app shows only the gate (`crate::screens::gate`), so this panel always has an account.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, Interval},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::cloud::CloudAddress;

use super::{
  Invites, Said,
  advanced::{self, Place},
  invites,
  parts::{Fact, body, frame, header, paragraph, section, section_title, tabs},
  words::{Look, cloud_error, counted, day_and_time},
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  theme::{ExtraRadius, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::{self, PropertyValue},
    state::loading,
    tabs::PageTab,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub account: Option<StoredCloudAccount>,
  pub address: CloudAddress,
  pub invites: Invites,
  /// The invites accepted here whose join did not finish.
  pub accepted: super::Kept,
  pub panel: OrchesterPanel,
}

impl DevtoolsInspectable for Props {}

pub(super) struct CloudPanel {
  advanced: advanced::Advanced,
  invites: invites::Answers,
  sign_out: FutureAction<(), (), Said>,
  /// *Sign out* asks before it signs out and locks.
  asking: Signal<bool>,
  /// Signing this computer out of the cloud is a person's (review 1, M1).
  person: super::PersonOnly,
  /// Counts the checks of joins that wait for approval ([`super::accepted::check`]).
  tick: Signal<u64>,
  checks: Interval,
}

const SIGN_OUT: &str = "orchester-cloud-sign-out";
const SIGN_OUT_CONFIRM: &str = "orchester-cloud-sign-out-confirm";

impl Component for CloudPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    let tick = ctx.signal(0_u64);
    let next = tick.clone();
    let checks = ctx.create_interval(super::accepted::CHECK_EVERY, move || next.update(|tick| *tick += 1));
    Self {
      advanced: advanced::Advanced::new(ctx),
      invites: invites::Answers::new(ctx),
      sign_out: ctx.future_action(move |(): ()| {
        let cloud = cloud.clone();
        async move { cloud.sign_out().await.map_err(|error| cloud_error(&error)) }
      }),
      asking: ctx.signal(false),
      person: super::PersonOnly::new(ctx),
      tick,
      checks,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.checks.start();
    if props.account.is_some() {
      let cloud = Services::of(ctx).cloud;
      super::accepted::check(ctx, self.tick.get(), (&cloud, &self.invites));
    }
    // Only while the window turns to the sign-in gate after a sign-out.
    let Some(account) = &props.account else {
      return Element::from(loading("orchester-cloud-reading", &ctx.t("cloud.reading")));
    };
    let look = Look {
      icon: Icon::CircleCheck,
      icon_color: lurq::app::theme::PaletteColor::Success,
      key: "cloud.state.signedIn",
      color: lurq::app::theme::PaletteColor::TextSecondary,
      name: "signed-in",
    };
    let facts = [
      Fact::Plain(ctx.t("cloud.list.cloud").to_string()),
      Fact::Mono(account.email.clone()),
    ];
    let sign_out = {
      let asking = self.asking.clone();
      ActionButton::new(SIGN_OUT, ButtonKind::Secondary, ctx.t("cloud.account.signOut"))
        .icon(Icon::LogOut)
        .held(self.asking.get())
        .on_click(move || asking.set(true))
    };
    let head = header(
      ctx,
      "orchester-cloud-header",
      (Icon::User, &account.display_name, &look),
      &facts,
    )
    .child(sign_out);
    let mut parts: Vec<Element> = Vec::new();
    parts.extend(match props.panel {
      OrchesterPanel::Invites => vec![invites::waiting(ctx, (&props.invites, &props.accepted), &self.invites)],
      _ => self.account_parts(ctx, account, &props),
    });
    frame("orchester-cloud-details")
      .child(head)
      .child(detail_tabs(ctx, &props))
      .child(body("orchester-cloud-body", parts))
      .into()
  }
}

impl CloudPanel {
  fn account_parts(&self, ctx: &mut Ctx, account: &StoredCloudAccount, props: &Props) -> Vec<Element> {
    let failed = self
      .sign_out
      .state()
      .get()
      .error
      .map(|reason| crate::ui::field::field_error("orchester-cloud-sign-out-error", &reason.text(ctx)).into());
    let this_device = ctx.t_args(
      "cloud.account.deviceValue",
      [("device", orchester_workspace::sharing::device::this_computer())],
    );
    let display_name = ctx.t_args("cloud.account.nameValue", [("name", account.display_name.as_str())]);
    let facts = Column::new()
      .id("orchester-cloud-facts")
      .width(Dimension::Pct(100.0))
      .child(inspector::property(
        "orchester-cloud-email",
        &ctx.t("cloud.account.signedInAs"),
        &account.email,
        PropertyValue::Plain,
      ))
      .child(inspector::property(
        "orchester-cloud-name",
        &ctx.t("cloud.account.displayName"),
        &display_name,
        PropertyValue::Plain,
      ))
      .child(inspector::property(
        "orchester-cloud-device",
        &ctx.t("cloud.account.thisDevice"),
        &this_device,
        PropertyValue::Plain,
      ))
      .child(inspector::property(
        "orchester-cloud-signed-in",
        &ctx.t("cloud.account.signedInAt"),
        &ctx.t_args(
          "cloud.account.signedInValue",
          [("when", day_and_time(ctx, account.signed_in_at))],
        ),
        PropertyValue::Plain,
      ))
      .child(inspector::property(
        "orchester-cloud-address",
        &ctx.t("cloud.account.address"),
        &account.cloud_address,
        PropertyValue::Mono,
      ));
    let signing_out = section(
      "orchester-cloud-signing-out",
      section_title(
        "orchester-cloud-signing-out-title",
        &ctx.t("cloud.account.signingOut"),
        None,
      ),
      vec![if self.asking.get() {
        self.sign_out_question(ctx)
      } else {
        paragraph(
          "orchester-cloud-signing-out-text",
          &ctx.t("cloud.account.signingOutText"),
        )
        .into()
      }],
    );
    let mut parts: Vec<Element> = failed.into_iter().collect();
    parts.push(facts.into());
    parts.push(invites::waiting(ctx, (&props.invites, &props.accepted), &self.invites));
    parts.push(signing_out.into());
    parts.push(advanced::section(ctx, &props.address, &self.advanced, Place::Settings));
    parts.push(inspector::note("orchester-cloud-token-note", Icon::KeyRound, &ctx.t(token_note())).into());
    parts
  }

  /// *Sign out of Orchester Cloud on this computer?* in place, with what locking means for the window, its MCP tools
  /// and the work here.
  fn sign_out_question(&self, ctx: &mut Ctx) -> Element {
    let (action, person) = (self.sign_out.clone(), self.person.clone());
    let confirm = ActionButton::new(
      SIGN_OUT_CONFIRM,
      ButtonKind::Danger,
      ctx.t("cloud.account.signOutConfirm.confirm"),
    )
    .icon(Icon::LogOut);
    let confirm = if self.sign_out.state().get().is_pending() {
      confirm.loading()
    } else {
      confirm.on_click(move || {
        if person.allows(SIGN_OUT_CONFIRM) {
          action.run(());
        }
      })
    };
    let stay = {
      let asking = self.asking.clone();
      ActionButton::new(
        "orchester-cloud-sign-out-stay",
        ButtonKind::Secondary,
        ctx.t("cloud.account.signOutConfirm.stay"),
      )
      .on_click(move || asking.set(false))
    };
    Column::new()
      .id("orchester-cloud-sign-out-question")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        crate::theme::ExtraSpacing::Inset.role(),
        SpacingSize::Md,
      ))
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(ExtraRadius::Tile.role())
      .child(
        Text::new(&ctx.t("cloud.account.signOutConfirm.question"))
          .variant(ExtraTypography::FormSection.role())
          .width(Dimension::Pct(100.0)),
      )
      .child(paragraph(
        "orchester-cloud-sign-out-locks",
        &ctx.t("cloud.account.signOutConfirm.locks"),
      ))
      .child(paragraph(
        "orchester-cloud-sign-out-local",
        &ctx.t("cloud.account.signOutConfirm.local"),
      ))
      .child(
        Row::new()
          .padding(Padding::new().top(SpacingSize::Xs))
          .spacing(SpacingSize::Sm)
          .child(confirm)
          .child(stay),
      )
      .with_children(self.person.reason(ctx, SIGN_OUT_CONFIRM))
      .into()
  }
}

fn detail_tabs(ctx: &mut Ctx, props: &Props) -> Element {
  let navigation = Navigation::of(ctx);
  let to = |panel: OrchesterPanel| {
    let navigation = navigation.clone();
    move || navigation.replace(Route::Orchester(OrchesterView { server: None, panel }))
  };
  let waiting = props.invites.waiting();
  let invites_label = if waiting > 0 {
    counted(
      ctx,
      ("cloud.account.tabs.invites.one", "cloud.account.tabs.invites.other"),
      waiting,
    )
  } else {
    ctx.t("cloud.account.tabs.invitesNone").to_string()
  };
  let entries = [
    PageTab::new(
      "orchester-cloud-tab-account",
      ctx.t("cloud.account.tabs.account"),
      props.panel != OrchesterPanel::Invites,
      to(OrchesterPanel::Account),
    ),
    PageTab::new(
      "orchester-cloud-tab-invites",
      invites_label,
      props.panel == OrchesterPanel::Invites,
      to(OrchesterPanel::Invites),
    ),
    PageTab::unavailable(
      "orchester-cloud-tab-devices",
      ctx.t("cloud.account.tabs.devices"),
      ctx.t("cloud.account.tabs.devicesNotBuilt"),
    ),
  ];
  tabs(ctx, "orchester-cloud-tabs", entries)
}

/// Where this platform keeps the cloud token, as the note under the account says it.
#[cfg(target_os = "macos")]
pub(super) const fn token_note() -> &'static str {
  "cloud.account.tokenNote.macos"
}

#[cfg(not(target_os = "macos"))]
pub(super) const fn token_note() -> &'static str {
  "cloud.account.tokenNote.windows"
}
