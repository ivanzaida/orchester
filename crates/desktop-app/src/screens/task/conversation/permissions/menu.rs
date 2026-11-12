//! The run's permissions menu (`Permissions menu` `WAMn1` in 15b `G1Vm1e`, rows in 15f `CimC4`): 400 wide,
//! `surface_raised` in a `border_strong` stroke, radius 12, 6 inside and 2 between rows, the large shadow. Under the
//! heading *Run 9 · how Builder's requests are answered* (26 high, a 12 shield, 11/500 `text_muted`) the three ways
//! the run can answer: a 14 glyph, the name in 12.5/500 and what it does in 11.5 `text_muted` 2 under it, a tag or a
//! check at the end; 6 over and under, 8 at the sides, 10 apart, the current one on `hover`. A way that cannot be
//! chosen has its name in `text_muted` and says why after an 11 lock in `text_secondary`. *Approve everything in this
//! run* asks in place first (`Confirm · approve everything` `GBhaV`), with the approval card's words. Then the run's
//! rules of Approve similar with *Revoke*, and the foot: what the profile and the project say, or why nothing is
//! wider, or that only a person widens a run, with a link to the profile.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  permission::ApprovalMode,
  run::{Answerer, ApprovalRule, EffectiveMode, ManualBecause, PermissionEventKind, RunPermissionChange},
};
use orchester_workspace::{
  display,
  tasks::run_permissions::{ChipMode, NotAutopilot, Offer, PermissionsMenu},
};

use super::{ChipProps, Permissions};
use crate::{
  navigation::Navigation,
  routes::{AgentTab, ProjectPage, Route},
  screens::task::conversation::timeline::rule_text,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, PermissionsSpacing, PermissionsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    menu::separator,
    message::message_text,
    pressable::{inert, pressable},
  },
};

/// The menu's width (`WAMn1`) and a heading's height (`m7aak`, `p0SAXr`).
pub(super) const WIDTH: f32 = 400.0;
const HEADING_HEIGHT: f32 = 26.0;

/// What a way of answering shows at its end.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tag {
  None,
  Narrows,
  OnlyYou,
  Check,
}

/// One way the run can answer, as its row shows it.
struct Way {
  id: &'static str,
  glyph: Icon,
  name: Arc<str>,
  /// What it does, or why it cannot be chosen.
  sub: Arc<str>,
  offer: Offer,
}

pub(super) fn panel(
  ctx: &mut Ctx,
  (props, shown): (&ChipProps, &PermissionsMenu),
  (permissions, confirming): (&Permissions, &Signal<bool>),
) -> Element {
  let run = props.run.1.to_string();
  let heading = ctx.t_args(
    "conversation.permissions.menu.heading",
    [("run", run.as_str()), ("agent", &*props.agent)],
  );
  if shown.chip == ChipMode::Dangerous {
    return super::dangerous::panel(ctx, props, &heading);
  }
  let everything = if confirming.get() {
    confirm(ctx, props, permissions, confirming)
  } else {
    way_row(
      ctx,
      &everything_way(ctx, props, shown),
      props,
      (permissions, confirming),
    )
  };
  let failure = permissions.failure().map(|message| {
    let reason = message_text(ctx, &message);
    field_error(
      "conversation-permissions-error",
      &ctx.t_args("conversation.permissions.failed", [("reason", &*reason)]),
    )
  });
  Column::new()
    .id("conversation-permissions-menu")
    .width(Dimension::Px(WIDTH))
    .padding(ExtraSpacing::Snug.role())
    .spacing(ExtraSpacing::Tight.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Lg)
    .child(heading_row(
      "conversation-permissions-heading",
      Some(Icon::Shield),
      &heading,
    ))
    .child(way_row(
      ctx,
      &manual_way(ctx, props, shown),
      props,
      (permissions, confirming),
    ))
    .with_children(
      (shown.autopilot != Offer::Hidden)
        .then(|| way_row(ctx, &autopilot_way(ctx, props, shown), props, (permissions, confirming))),
    )
    .child(everything)
    .child(separator(ctx))
    .child(heading_row(
      "conversation-permissions-similar",
      None,
      &ctx.t("conversation.permissions.similar.heading"),
    ))
    .with_children(similar_rules(ctx, props, permissions))
    .with_children(failure)
    .child(foot(ctx, props, shown))
    .into()
}

pub(super) fn heading_row(id: &str, glyph: Option<Icon>, words: &str) -> Element {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADING_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Sm, PaletteColor::TextMuted)))
    .child(Text::new(words).variant(PermissionsText::Heading.role()).nowrap())
    .into()
}

fn manual_way(ctx: &Ctx, props: &ChipProps, shown: &PermissionsMenu) -> Way {
  let approvals = &props.approvals;
  let run = props.run.1.to_string();
  let sub = match (shown.manual, approvals.mode, approvals.switched) {
    (Offer::Change(RunPermissionChange::StopApprovingEverything), ..) => ctx.t_args(
      "conversation.permissions.option.manualRun.subStop",
      [("agent", &*props.agent)],
    ),
    (Offer::Current, EffectiveMode::Manual(ManualBecause::Run), Some(switch)) => {
      let key = match switch.by {
        Answerer::Person => "conversation.permissions.option.manualRun.subSwitchedPerson",
        Answerer::Automation => "conversation.permissions.option.manualRun.subSwitchedAutomation",
      };
      ctx.t_args(key, [("time", display::clock(switch.at).as_str())])
    }
    (Offer::Current, ..) => ctx.t("conversation.permissions.option.manualRun.subCurrent"),
    _ => ctx.t_args("conversation.permissions.option.manualRun.sub", [("run", run.as_str())]),
  };
  Way {
    id: "conversation-permissions-manual",
    glyph: Icon::Shield,
    name: ctx.t("conversation.permissions.option.manualRun.name"),
    sub,
    offer: shown.manual,
  }
}

fn autopilot_way(ctx: &Ctx, props: &ChipProps, shown: &PermissionsMenu) -> Way {
  let sub = match shown.autopilot {
    Offer::Change(_) => ctx.t("conversation.permissions.option.autopilot.subStop"),
    Offer::Unavailable(why) => unavailable(ctx, props, why),
    Offer::Current | Offer::Hidden => ctx.t("conversation.permissions.option.autopilot.sub"),
  };
  Way {
    id: "conversation-permissions-autopilot",
    glyph: Icon::ShieldCheck,
    name: ctx.t("conversation.permissions.option.autopilot.name"),
    sub,
    offer: shown.autopilot,
  }
}

fn everything_way(ctx: &Ctx, props: &ChipProps, shown: &PermissionsMenu) -> Way {
  let run = props.run.1.to_string();
  let since = props
    .approvals
    .events
    .iter()
    .rev()
    .find(|event| matches!(event.kind, PermissionEventKind::Everything { .. }));
  let sub = match (shown.everything, since) {
    (Offer::Current, Some(event)) => ctx.t_args(
      "conversation.permissions.option.everything.subOn",
      [("time", display::clock(event.at).as_str())],
    ),
    (Offer::Current, None) => ctx.t("conversation.permissions.option.everything.subOnUntimed"),
    _ => ctx.t_args(
      "conversation.permissions.option.everything.sub",
      [("run", run.as_str())],
    ),
  };
  Way {
    id: "conversation-permissions-everything",
    glyph: Icon::ShieldAlert,
    name: ctx.t("conversation.permissions.option.everything.name"),
    sub,
    offer: shown.everything,
  }
}

/// Why Autopilot cannot be chosen for the run.
fn unavailable(ctx: &Ctx, props: &ChipProps, why: NotAutopilot) -> Arc<str> {
  let key = match why {
    NotAutopilot::Switched => "conversation.permissions.unavailable.switched",
    NotAutopilot::ProfileManual => "conversation.permissions.unavailable.profileManual",
    NotAutopilot::ProjectOff => "conversation.permissions.unavailable.projectOff",
  };
  ctx.t_args(key, [("agent", &*props.agent), ("project", &*props.project.1)])
}

/// A way's row: its glyph, name and what it does, and its tag or check; chosen, it makes its change, or asks first.
fn way_row(ctx: &Ctx, way: &Way, props: &ChipProps, presses: (&Permissions, &Signal<bool>)) -> Element {
  let unavailable = matches!(way.offer, Offer::Unavailable(_));
  let tag = match way.offer {
    Offer::Current => Tag::Check,
    Offer::Change(change) if change.widens() => Tag::OnlyYou,
    Offer::Change(_) => Tag::Narrows,
    Offer::Unavailable(_) | Offer::Hidden => Tag::None,
  };
  let (name_color, glyph_color) = if unavailable {
    (PaletteColor::TextMuted, PaletteColor::TextMuted)
  } else {
    (PaletteColor::TextPrimary, PaletteColor::TextSecondary)
  };
  let content = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(
      Column::new()
        .padding(Padding::new().top(PermissionsSpacing::Glyph.role()))
        .child(icon(way.glyph, IconSize::Md, glyph_color)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&way.name)
            .id(format!("{}-name", way.id))
            .variant(ExtraTypography::OptionTitle.role())
            .color(name_color),
        )
        .child(sub_line(way, unavailable)),
    )
    .with_children(tag_of(ctx, tag));
  let row = way_press(ctx, way, props, presses)
    .id(way.id)
    .describe("a11y_name", way.name.clone())
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Md)
    .child(content);
  match way.offer {
    Offer::Current => row.background(ExtraColor::Hover.role()).into(),
    Offer::Change(_) => row
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .into(),
    Offer::Unavailable(_) | Offer::Hidden => row.into(),
  }
}

/// What a way does, under its name; why it cannot be chosen after a lock, when it cannot.
fn sub_line(way: &Way, unavailable: bool) -> Element {
  if !unavailable {
    return Text::new(&way.sub)
      .id(format!("{}-sub", way.id))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
      .into();
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Column::new()
        .padding(Padding::new().top(ExtraSpacing::Tight.role()))
        .child(icon(Icon::Lock, IconSize::Xs, PaletteColor::TextSecondary)),
    )
    .child(
      Text::new(&way.sub)
        .id(format!("{}-reason", way.id))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// What a press on a way does: nothing more than closing the menu for the current one; asking in place first for
/// *Approve everything in this run*; its change for another; nothing for one that cannot be chosen.
fn way_press(
  ctx: &Ctx,
  way: &Way,
  props: &ChipProps,
  (permissions, confirming): (&Permissions, &Signal<bool>),
) -> lurq::components::Button {
  match way.offer {
    Offer::Unavailable(_) | Offer::Hidden => inert(lurq::components::Button::empty())
      .describe("state", "unavailable")
      .describe("reason", way.sub.clone()),
    Offer::Current => {
      let open = permissions.open.clone();
      pressable(move || open.set(false)).describe("state", "current")
    }
    Offer::Change(RunPermissionChange::ApproveEverything) => {
      let confirming = confirming.clone();
      pressable(move || confirming.set(true))
        .describe("state", "enabled")
        .describe("person_only", ctx.t("conversation.permissions.unavailable.personOnly"))
    }
    Offer::Change(change) => {
      let (permissions, root, run) = (permissions.clone(), props.root.clone(), props.run.0);
      pressable(move || {
        permissions.open.set(false);
        permissions.ask(&root, run, change);
      })
      .describe("state", "enabled")
    }
  }
}

fn tag_of(ctx: &Ctx, tag: Tag) -> Option<Element> {
  let words = |key: &str| Text::new(&ctx.t(key)).variant(PermissionsText::Tag.role()).nowrap();
  let element: Element = match tag {
    Tag::None => return None,
    Tag::Check => Column::new()
      .padding(Padding::new().top(PermissionsSpacing::Glyph.role()))
      .child(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary))
      .into(),
    Tag::Narrows => Row::new()
      .padding(Padding::new().top(ExtraSpacing::Tight.role()))
      .child(words("conversation.permissions.tag.narrows"))
      .into(),
    Tag::OnlyYou => Row::new()
      .padding(Padding::new().top(ExtraSpacing::Tight.role()))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(icon(Icon::UserRound, IconSize::Xs, PaletteColor::TextMuted))
      .child(words("conversation.permissions.tag.onlyYou"))
      .into(),
  };
  Some(element)
}

/// *Approve everything in this run* asking in place: the approval card's question, *Keep asking* and *Approve
/// everything*.
fn confirm(ctx: &mut Ctx, props: &ChipProps, permissions: &Permissions, confirming: &Signal<bool>) -> Element {
  let run = props.run.1.to_string();
  let keep = {
    let confirming = confirming.clone();
    ActionButton::new(
      "conversation-permissions-confirm-keep",
      ButtonKind::Ghost,
      ctx.t("cards.approval.confirm.keep"),
    )
    .on_click(move || confirming.set(false))
  };
  let approve = {
    let (permissions, root, run) = (permissions.clone(), props.root.clone(), props.run.0);
    let confirming = confirming.clone();
    ActionButton::new(
      "conversation-permissions-confirm-approve",
      ButtonKind::Primary,
      ctx.t("cards.approval.confirm.everything"),
    )
    .on_click(move || {
      confirming.set(false);
      permissions.open.set(false);
      permissions.ask(&root, run, RunPermissionChange::ApproveEverything);
    })
  };
  Column::new()
    .id("conversation-permissions-confirm")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .background(ExtraColor::Hover.role())
    .rounded(RadiusSize::Md)
    .child(
      Text::new(&ctx.t_args("cards.approval.confirm.everythingTitle", [("run", run.as_str())]))
        .id("conversation-permissions-confirm-title")
        .variant(ExtraTypography::OptionTitle.role()),
    )
    .child(
      Text::new(&ctx.t("conversation.permissions.option.everything.confirmBody"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(Row::new().spacing(SpacingSize::Sm).child(keep).child(approve))
    .into()
}

/// The run's rules of Approve similar that answer still, each with *Revoke*; a line when there are none.
fn similar_rules(ctx: &Ctx, props: &ChipProps, permissions: &Permissions) -> Vec<Element> {
  let rules: Vec<&ApprovalRule> = props
    .approvals
    .rules
    .iter()
    .filter(|rule| rule.is_active() && rule.scope.revocable())
    .collect();
  if rules.is_empty() {
    let empty = Row::new()
      .id("conversation-permissions-similar-empty")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Pair.role()))
      .child(
        Text::new(&ctx.t("conversation.permissions.similar.empty"))
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      );
    return vec![empty.into()];
  }
  rules
    .into_iter()
    .map(|rule| similar_rule(ctx, props, permissions, rule))
    .collect()
}

fn similar_rule(ctx: &Ctx, props: &ChipProps, permissions: &Permissions, rule: &ApprovalRule) -> Element {
  let (words, detail) = rule_text(ctx, &rule.scope, rule.created_at);
  let id = format!("conversation-permissions-rule-{}", rule.id.get());
  let (permissions, root, run, rule_id) = (permissions.clone(), props.root.clone(), props.run.0, rule.id);
  let revoke = pressable(move || permissions.ask(&root, run, RunPermissionChange::Revoke(rule_id)))
    .id(format!("{id}-revoke"))
    .describe("state", "enabled")
    .child(
      Text::new(&ctx.t("conversation.permissions.similar.revoke"))
        .variant(PermissionsText::Revoke.role())
        .nowrap(),
    );
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Pair.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .child(icon(Icon::CheckCheck, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&words).variant(ExtraTypography::PropertyMono.role()).nowrap())
    .child(
      Text::new(&detail)
        .variant(PermissionsText::Chip.role())
        .color(PaletteColor::TextMuted)
        .nowrap()
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(revoke)
    .into()
}

/// The menu's foot: that only a person widens a run, after automation was refused; else why nothing is wider, or what
/// the profile and the project say; and a link to the profile.
fn foot(ctx: &mut Ctx, props: &ChipProps, shown: &PermissionsMenu) -> Element {
  let approvals = &props.approvals;
  let refused = approvals
    .events
    .last()
    .is_some_and(|event| event.kind == PermissionEventKind::Refused);
  let allows = approvals.allow_autopilot.is_on();
  let key = match (refused, shown.nothing_wider, shown.profile, allows) {
    (true, ..) => "conversation.permissions.foot.personOnly",
    (false, Some(NotAutopilot::ProfileManual), _, false) => {
      "conversation.permissions.foot.nothingWider.profileManualProjectOff"
    }
    (false, Some(NotAutopilot::ProfileManual), ..) => "conversation.permissions.foot.nothingWider.profileManual",
    (false, Some(NotAutopilot::ProjectOff), ..) => "conversation.permissions.foot.nothingWider.projectOff",
    (false, Some(NotAutopilot::Switched), ..) => "conversation.permissions.foot.nothingWider.switched",
    (false, None, ApprovalMode::Manual, true) => "conversation.permissions.foot.manualAllows",
    (false, None, ApprovalMode::Manual, false) => "conversation.permissions.foot.manualOff",
    (false, None, ApprovalMode::Autopilot, true) => "conversation.permissions.foot.autopilotAllows",
    (false, None, ApprovalMode::Autopilot, false) => "conversation.permissions.foot.autopilotOff",
  };
  let run = props.run.1.to_string();
  let note = ctx.t_args(
    key,
    [
      ("agent", &*props.agent),
      ("project", &*props.project.1),
      ("run", run.as_str()),
    ],
  );
  let link = props.profile.filter(|_| !refused).map(|profile| {
    let open = Navigation::of(ctx).to(Route::Project(
      props.project.0,
      ProjectPage::Agent(profile, AgentTab::default()),
    ));
    let label = ctx.t_args("conversation.permissions.foot.profile", [("agent", &*props.agent)]);
    Element::from(
      ActionButton::new("conversation-permissions-profile-link", ButtonKind::Ghost, label)
        .icon(Icon::ExternalLink)
        .opens(open),
    )
  });
  Row::new()
    .id("conversation-permissions-foot")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .left(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Text::new(&note)
        .id("conversation-permissions-foot-note")
        .variant(ExtraTypography::Note.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(link)
    .into()
}
