//! The run permissions popover (02.J `NNRH8`): which agent, step, model and profile version the
//! permissions come from, what the current or last invocation launched with, when a change takes
//! effect (`aDPaV`), the settings for the next invocation (`settings`),
//! *Also save to the profile* (`u8JAYJ`) and the footer: Cancel, Apply now (`v2ssd`) and Apply from
//! next invocation (`JOLcW`); under the settings, the MCP servers the agent gets (read-only). It is a form: Tab moves
//! through its controls and wraps, Enter applies from the next invocation, Escape or Cancel closes it without changes.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Rect, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::run::PermissionsApply;

use super::{
  super::heading::agent_of, ApplyAction, ApplyRequest, RunPermissionsProps, RunPermissionsRow, settings::settings,
};
use crate::{
  screens::project::connections::status::short_key,
  tasks::permissions::InvocationPolicy,
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography, UNAVAILABLE_OPTION_OPACITY},
  ui::{
    button::ActionButton,
    field::field_error,
    form::{Submit, form},
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop, pressable, surface},
  },
};

/// The MCP servers the agent gets, read-only: they come from the profile version and are not changed for
/// a run (`mKDBS` in 02.J `NNRH8`).
fn mcp_servers(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let servers = &props.permissions.mcp_servers;
  let text = if servers.is_empty() {
    ctx.t("runs.permissions.mcp.none")
  } else {
    let names: Vec<&str> = servers.iter().map(|name| &**name).collect();
    ctx.t_args(
      "runs.permissions.mcp.servers",
      [("servers", names.join(&ctx.t("common.listSeparator")).as_str())],
    )
  };
  Column::new()
    .id("task-permissions-mcp")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(&ctx.t("runs.permissions.mcp.label")).variant(TypographyStyle::Caption))
    .child(
      Text::new(&text)
        .variant(ExtraTypography::MonoValue.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(Text::new(&ctx.t("runs.permissions.mcp.fromProfile")).variant(ExtraTypography::Hint.role()))
    .into()
}

/// Popover width (`NNRH8`).
const POPOVER_WIDTH: f32 = 460.0;
const DIVIDER_HEIGHT: f32 = 1.0;
/// Button height in the popover footer (`lTaB5`).
const FOOTER_BUTTON_HEIGHT: f32 = 30.0;

type Apply = Arc<dyn Fn(PermissionsApply) + Send + Sync>;

impl RunPermissionsRow {
  pub(super) fn popover(&self, ctx: &Ctx, props: &RunPermissionsProps, action: &ApplyAction) -> Element {
    let Some(draft) = self.edit.draft.get().0 else {
      return Column::new().id("task-permissions-popover").into();
    };
    let state = action.state().get();
    let pending = state.is_pending();
    let changed = draft != props.permissions.base;
    let apply: Option<Apply> = (props.live && changed && !pending).then(|| {
      let (action, edit, props) = (action.clone(), self.edit.clone(), props.clone());
      Arc::new(move |apply| {
        let Some(permissions) = edit.draft.get_untracked().0 else {
          return;
        };
        action.run(ApplyRequest {
          run: props.run,
          profile: props.permissions.profile_id,
          permissions,
          apply,
          save_to: edit.save.get_untracked().then(|| props.repo_root.clone()),
        });
      }) as Apply
    });
    let failure = state
      .error
      .filter(|_| !pending)
      .map(|error| field_error(&message_text(ctx, &error)));
    let content = Column::new()
      .id("task-permissions-popover")
      .width(Dimension::Px(POPOVER_WIDTH))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .box_shadow(ExtraShadow::Popover.role())
      .overflow_visible()
      .child(self.header(ctx, props))
      .with_children(current_policy(ctx, props))
      .child(takes_effect(ctx, props))
      .child(Text::new(&ctx.t("runs.permissions.desiredLabel")).variant(TypographyStyle::Label))
      .with_children(settings(ctx, &self.edit, props, &draft))
      .child(mcp_servers(ctx, props))
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()))
      .child(self.save_to_profile(ctx, props))
      .with_children(failure)
      .child(self.footer(ctx, props, apply.as_ref(), pending));
    let submit: Submit = apply.map(|apply| Arc::new(move || apply(PermissionsApply::NextInvocation)) as _);
    form(submit, content)
  }

  /// "Permissions for this run" over "Builder · Implement · Codex / Sol · from profile v2", and ×.
  fn header(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let subtitle = ctx.t_args(
      "runs.permissions.popover.subtitle",
      [
        ("agent", agent_of(&props.step)),
        ("step", &*props.step.name),
        (
          "connection",
          &*props.step.provider.map_or_else(
            || ctx.t("pipelines.inspector.permissions.none"),
            |provider| ctx.t(short_key(provider)),
          ),
        ),
        ("model", props.step.model.as_deref().unwrap_or_default()),
        ("version", props.permissions.profile.to_string().as_str()),
      ],
    );
    let close = self.open.clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("runs.permissions.popover.title")).variant(TypographyStyle::Label))
          .child(Text::new(&subtitle).variant(TypographyStyle::Caption)),
      )
      .child(
        pressable(surface().id("task-permissions-close"), move || close.set(false))
          .rounded(RadiusSize::Sm)
          .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary)),
      )
      .into()
  }

  /// *Also save to the Builder profile* (`u8JAYJ`): a checkbox with where the change goes.
  fn save_to_profile(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let checked = self.edit.save.get();
    let glyph = if checked { Icon::SquareCheck } else { Icon::Square };
    let draft = props.permissions.draft.to_string();
    let row = Button::empty()
      .button()
      .id("task-permissions-save-to-profile")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Start)
      .rounded(RadiusSize::Sm)
      .describe("checked", if checked { "true" } else { "false" })
      .child(icon(glyph, IconSize::Event, PaletteColor::TextSecondary))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Mark.role())
          .child(
            Text::new(&ctx.t_args(
              "runs.permissions.popover.saveToProfile",
              [("agent", agent_of(&props.step))],
            ))
            .variant(TypographyStyle::FieldLabel)
            .color(PaletteColor::TextPrimary),
          )
          .child(
            Text::new(&ctx.t_args(
              "runs.permissions.popover.saveToProfileDetail",
              [("version", draft.as_str())],
            ))
            .variant(ExtraTypography::Hint.role()),
          ),
      );
    if !props.live {
      return row
        .focusable(false)
        .opacity(UNAVAILABLE_OPTION_OPACITY)
        .cursor(CursorIcon::NotAllowed)
        .describe("state", "unavailable")
        .into();
    }
    let save = self.edit.save.clone();
    keyboard_stop(row, Ring::Outer)
      .cursor(CursorIcon::Pointer)
      .describe("state", "available")
      .on_click(move |_| save.update(|save| *save = !*save))
      .into()
  }

  /// Cancel closes; Apply now while an agent of the profile is at work; Apply from next
  /// invocation (`dIP0h`). Both apply only a change.
  fn footer(&self, ctx: &Ctx, props: &RunPermissionsProps, apply: Option<&Apply>, pending: bool) -> Element {
    let close = self.open.clone();
    let height = Dimension::Px(FOOTER_BUTTON_HEIGHT);
    let button = |label: &str, id: &'static str, glyph: Icon, how: Option<PermissionsApply>| {
      let button = ActionButton::new(ctx.t(label))
        .id(id)
        .icon(if pending { Icon::Loader } else { glyph })
        .compact()
        .in_form()
        .height(height);
      match (apply, how) {
        (Some(apply), Some(how)) => {
          let apply = Arc::clone(apply);
          button.on_click(move || apply(how))
        }
        _ => button.disabled(),
      }
    };
    let now = props.permissions.can_apply_now.then_some(PermissionsApply::Now);
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .justify(Justify::End)
      .align_items(Alignment::Center)
      .child(
        ActionButton::new(ctx.t("common.cancel"))
          .id("task-permissions-cancel")
          .compact()
          .in_form()
          .height(height)
          .on_click(move || close.set(false))
          .render(ctx),
      )
      .child(
        button(
          "runs.permissions.popover.applyNow",
          "task-permissions-apply-now",
          Icon::RefreshCw,
          now,
        )
        .render(ctx),
      )
      .child(
        button(
          "runs.permissions.popover.applyNext",
          "task-permissions-apply-next",
          Icon::Check,
          Some(PermissionsApply::NextInvocation),
        )
        .primary()
        .render(ctx),
      )
      .into()
  }
}

/// What the step's current or last invocation launched with. Before any has launched, the settings
/// under *Next invocation settings* are what it will launch with, so the line would only repeat them.
fn current_policy(ctx: &Ctx, props: &RunPermissionsProps) -> Option<Element> {
  (props.permissions.invocation_policy != InvocationPolicy::NoInvocation).then(|| {
    Text::new(&super::value(ctx, &props.step, &props.permissions))
      .id("task-permissions-current-policy")
      .variant(TypographyStyle::Caption)
      .into()
  })
}

/// When a change takes effect (`XpncF`, `aDPaV`).
fn takes_effect(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let run = props.run_number.to_string();
  let text = if props.live {
    ctx.t_args(
      "runs.permissions.popover.takesEffect",
      [
        ("agent", agent_of(&props.step)),
        ("step", &*props.step.name),
        ("run", run.as_str()),
      ],
    )
  } else {
    ctx.t("runs.permissions.completedDetail")
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .background(ExtraColor::Surface.role())
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::Clock, IconSize::Event, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::Detail.role()).flex(1.0))
    .into()
}
