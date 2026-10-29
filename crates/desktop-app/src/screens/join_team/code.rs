//! The code column (`With a code` in `B914L`): the team server's address and the code in Geist Mono, 36 high, what a
//! code is under it, and *Join with the code*, *Joining…* while it joins. A wrong or expired code says so in the hint's
//! place and marks its field (`aWsVx`); a server that does not answer says so under the button in a danger callout,
//! with *Try again* under it, and both fields keep what was typed (`oOn6j`). What the join came to shows under the
//! button. The column is a lurq form: Enter in either field joins as the button does.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Form as LurqForm, FormProps, Row},
  node::Element,
};
use orchester_api::client::CONNECT_TIMEOUT;

use super::{
  code_join::{CodeJoin, CodeRefusal, SUBMIT},
  outcome, ways,
};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::{ControlHeight, StartupText},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::{FieldInput, field, field_error},
    icon::Icon,
  },
};

const ID: &str = "join-team-code";

/// The column; the address takes the focus once when `focus` (the code page, signed in).
pub(super) fn column(ctx: &mut Ctx, join: &CodeJoin, focus: bool) -> Element {
  if focus && !join.focused.get() {
    join.focused.set(true);
    ctx.focus(&join.address_input);
  }
  let refusal = join.refusal();
  let head = ways::head(
    ID,
    Icon::KeyRound,
    (&ctx.t("startup.join.code.title"), &ctx.t("startup.join.code.sub")),
  );
  let mut parts = vec![address(ctx, join, refusal.as_ref()), code(ctx, join, refusal.as_ref())];
  parts.push(submit(ctx, join));
  parts.extend(join.person.reason(ctx, SUBMIT));
  parts.extend(refused(ctx, join, refusal.as_ref()));
  if let Some(outcome) = join.outcome() {
    parts.push(outcome::callout(ctx, ID, &outcome, None));
  }
  let submit = join.submitter();
  LurqForm::element(
    FormProps::default().on_submit_data(move |_| submit()),
    ways::way(ID, head, parts),
  )
}

/// The address, marked when it is no server address.
fn address(ctx: &mut Ctx, join: &CodeJoin, refusal: Option<&CodeRefusal>) -> Element {
  let label = ctx.t("startup.join.code.address");
  let mut input = FieldInput::new(format!("{ID}-address"), label.clone(), join.address.clone())
    .placeholder(ctx.t("startup.join.code.addressPlaceholder"))
    .text_style(StartupText::FieldMono.role())
    .box_height(ControlHeight::Lg.value())
    .element(join.address_input.clone());
  let invalid = refusal == Some(&CodeRefusal::AddressInvalid);
  if invalid {
    input = input.invalid();
  }
  let input = input.render(ctx);
  let mut part = field(&format!("{ID}-address-field"), Some(&label), input, None);
  if invalid {
    part = part.child(field_error(
      &format!("{ID}-address-invalid"),
      &ctx.t("startup.join.code.addressInvalid"),
    ));
  }
  part.into()
}

/// The code, with what a code is under it, or that it is wrong or expired in that place (`aWsVx`).
fn code(ctx: &mut Ctx, join: &CodeJoin, refusal: Option<&CodeRefusal>) -> Element {
  let label = ctx.t("startup.join.code.label");
  let wrong = refusal == Some(&CodeRefusal::WrongCode);
  let mut input = FieldInput::new(format!("{ID}-code"), label.clone(), join.code.clone())
    .placeholder(ctx.t("startup.join.code.placeholder"))
    .text_style(StartupText::FieldMono.role())
    .box_height(ControlHeight::Lg.value());
  if wrong {
    input = input.invalid();
  }
  let input = input.render(ctx);
  if wrong {
    return field(&format!("{ID}-code-field"), Some(&label), input, None)
      .child(field_error(&format!("{ID}-wrong"), &ctx.t("startup.join.code.wrong")))
      .into();
  }
  let hint = ctx.t("startup.join.code.hint");
  field(&format!("{ID}-code-field"), Some(&label), input, Some(&hint)).into()
}

/// *Join with the code*, *Joining…* while it joins.
fn submit(ctx: &Ctx, join: &CodeJoin) -> Element {
  if join.pending() {
    return ActionButton::new(SUBMIT, ButtonKind::Secondary, ctx.t("startup.join.code.joining"))
      .large()
      .fill_width()
      .loading()
      .into();
  }
  ActionButton::new(SUBMIT, ButtonKind::Secondary, ctx.t("startup.join.code.submit"))
    .icon(Icon::ArrowRight)
    .large()
    .fill_width()
    .in_form()
    .on_click(join.submitter())
    .into()
}

/// A refusal under the button: nothing answered, with *Try again* (`oOn6j`); a server that needs the person's check
/// first, with the way to *Connect a server*; another failure in its words. A wrong code or address is its field's.
fn refused(ctx: &mut Ctx, join: &CodeJoin, refusal: Option<&CodeRefusal>) -> Vec<Element> {
  match refusal {
    None | Some(CodeRefusal::WrongCode | CodeRefusal::AddressInvalid) => Vec::new(),
    Some(CodeRefusal::Unreachable { host }) => {
      let seconds = CONNECT_TIMEOUT.as_secs().to_string();
      let callout = Callout::new(
        format!("{ID}-unreachable"),
        CalloutTone::Danger,
        ctx.t_args("startup.join.code.unreachable.title", [("host", host.as_str())]),
      )
      .icon(Icon::WifiOff)
      .body(ctx.t_args("startup.join.code.unreachable.body", [("seconds", seconds)]));
      let again = ActionButton::new(
        format!("{ID}-try-again"),
        ButtonKind::Secondary,
        ctx.t("common.tryAgain"),
      )
      .icon(Icon::RotateCw)
      .on_click(join.submitter());
      let retry = Row::new()
        .id(format!("{ID}-retry"))
        .spacing(SpacingSize::Sm)
        .child(again);
      vec![callout.into(), retry.into()]
    }
    Some(CodeRefusal::NeedsSettings { server }) => {
      let open = ActionButton::new(
        format!("{ID}-open-connect"),
        ButtonKind::Secondary,
        ctx.t("startup.join.code.settings.open"),
      )
      .icon(Icon::ArrowRight)
      .opens(Navigation::of(ctx).to(Route::Orchester(OrchesterView::connect())));
      let callout = Callout::new(
        format!("{ID}-needs-settings"),
        CalloutTone::Warning,
        ctx.t_args("startup.join.code.settings.title", [("server", server.as_str())]),
      )
      .body(ctx.t("startup.join.code.settings.body"))
      .action(open);
      vec![callout.into()]
    }
    Some(CodeRefusal::Other(shown)) => vec![field_error(&format!("{ID}-failed"), &shown.text(ctx)).into()],
  }
}
