//! The Orchester Cloud account (`Account details` `S9wde` in `Cu20b`): the header with *Sign out*; the tabs *Account*,
//! *Invites* and *Devices* (not built in this app yet, and says so); the account's facts, the waiting invites, what
//! signing out does and the advanced cloud address. Signed out, the panel is the sign-in form ([`super::sign_in`]).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_store::StoredCloudAccount;
use orchester_workspace::sharing::cloud::CloudAddress;

use super::{
  Invites, Said, advanced, invites,
  parts::{body, frame, header, paragraph, section, section_title, tabs},
  sign_in,
  words::{Look, cloud_error, counted, day_and_time},
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::{self, PropertyValue},
    tabs::PageTab,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub account: Option<StoredCloudAccount>,
  pub address: CloudAddress,
  pub invites: Invites,
  pub panel: OrchesterPanel,
}

impl DevtoolsInspectable for Props {}

pub(super) struct CloudPanel {
  form: sign_in::Form,
  advanced: advanced::Advanced,
  invites: invites::Answers,
  sign_out: FutureAction<(), (), Said>,
}

impl Component for CloudPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let cloud = Services::of(ctx).cloud;
    Self {
      form: sign_in::Form::new(ctx),
      advanced: advanced::Advanced::new(ctx),
      invites: invites::Answers::new(ctx),
      sign_out: ctx.future_action(move |(): ()| {
        let cloud = cloud.clone();
        async move { cloud.sign_out().await.map_err(|error| cloud_error(&error)) }
      }),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Some(account) = &props.account else {
      return sign_in::panel(ctx, &props.address, &self.form, &self.advanced);
    };
    let look = Look {
      icon: Icon::CircleCheck,
      icon_color: lurq::app::theme::PaletteColor::Success,
      key: "cloud.state.signedIn",
      color: lurq::app::theme::PaletteColor::TextSecondary,
      name: "signed-in",
    };
    let facts = [ctx.t("cloud.list.cloud").to_string(), account.email.clone()];
    let sign_out = {
      let action = self.sign_out.clone();
      let button = ActionButton::new(
        "orchester-cloud-sign-out",
        ButtonKind::Secondary,
        ctx.t("cloud.account.signOut"),
      )
      .icon(Icon::LogOut);
      if self.sign_out.state().get().is_pending() {
        button.loading()
      } else {
        button.on_click(move || action.run(()))
      }
    };
    let head = header(
      ctx,
      "orchester-cloud-header",
      (Icon::User, &account.display_name, &look),
      &facts,
    )
    .child(sign_out);
    let parts = match props.panel {
      OrchesterPanel::Invites => vec![invites::waiting(ctx, &props.invites, &self.invites)],
      _ => self.account_parts(ctx, account, &props),
    };
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
      vec![
        paragraph(
          "orchester-cloud-signing-out-text",
          &ctx.t("cloud.account.signingOutText"),
        )
        .into(),
      ],
    );
    let mut parts: Vec<Element> = failed.into_iter().collect();
    parts.push(facts.into());
    parts.push(invites::waiting(ctx, &props.invites, &self.invites));
    parts.push(signing_out.into());
    parts.push(advanced::section(ctx, &props.address, &self.advanced));
    parts.push(inspector::note("orchester-cloud-token-note", Icon::KeyRound, &ctx.t(token_note())).into());
    parts
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
