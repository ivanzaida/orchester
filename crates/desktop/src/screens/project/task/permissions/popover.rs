//! The run permissions popover (02.J `NNRH8`): which agent, step, model and profile version the
//! permissions come from, what the agent runs with (`UPDBO`, wrapping at a "·" as `zCqp2`), when a
//! change takes effect (`aDPaV`), the settings for the next invocation (`settings`), *Also save to
//! the profile* (`u8JAYJ`) and the footer: Cancel, Apply now (`v2ssd`) and Apply from next
//! invocation (`JOLcW`); under the settings, the MCP servers the agent gets (read-only). It is a
//! form: Tab moves through its controls and wraps, Enter applies from the next invocation, Escape
//! or Cancel closes it without changes. For an ended run (04.2 `YgLEg`) the settings are read-only,
//! the notice says why, and only *Close* is left.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Rect, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::run::PermissionsApply;

use super::{
  super::heading::agent_of,
  ApplyAction, ApplyRequest, RunPermissionsProps, RunPermissionsRow,
  policy::{Wording, policy_property, policy_text},
  settings::settings,
};
use crate::{
  screens::project::connections::status::short_key,
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
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
  /// The popover at most `cap` tall: its header and footer stay whole and the body between them
  /// scrolls when the room above the row is too short (`ecCTw`).
  pub(super) fn popover(
    &self,
    ctx: &Ctx,
    props: &RunPermissionsProps,
    action: &ApplyAction,
    (cap, auto_mode): (f32, Element),
  ) -> Element {
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
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Group.role())
      .child(current_policy(ctx, props))
      .child(takes_effect(ctx, props))
      .with_children(settings(ctx, &self.edit, props, &draft))
      .child(mcp_servers(ctx, props))
      .child(auto_mode)
      .with_children(props.live.then(|| {
        Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role())
      }))
      .with_children(props.live.then(|| self.save_to_profile(ctx, props)))
      .with_children(failure);
    let content = Column::new()
      .id("task-permissions-popover")
      .width(Dimension::Px(POPOVER_WIDTH))
      .max_height(Dimension::Px(cap))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .box_shadow(ExtraShadow::Popover.role())
      .child(self.header(ctx, props))
      .child(
        ScrollVertical::new(body)
          .id("task-permissions-scroll")
          .width(Dimension::Pct(100.0))
          .flex_shrink(1.0),
      )
      .child(self.footer(ctx, props, apply.as_ref(), pending));
    let submit: Submit = apply.map(|apply| Arc::new(move || apply(PermissionsApply::NextInvocation)) as _);
    form(submit, content)
  }

  /// "Permissions for this run" over "Builder · Implement · Codex / Sol · from profile v2" (with
  /// "· run ended" for an ended run, `B4zwoA`), and ×.
  fn header(&self, ctx: &Ctx, props: &RunPermissionsProps) -> Element {
    let key = if props.live {
      "runs.permissions.popover.subtitle"
    } else {
      "runs.permissions.popover.subtitleEnded"
    };
    let subtitle = ctx.t_args(
      key,
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
    let save = self.edit.save.clone();
    keyboard_stop(row, Ring::Outer)
      .cursor(CursorIcon::Pointer)
      .describe("state", "available")
      .on_click(move |_| save.update(|save| *save = !*save))
      .into()
  }

  /// Cancel closes; Apply now while an agent of the profile is at work; Apply from next
  /// invocation (`dIP0h`). Both apply only a change. An ended run has only *Close* (`gjM7t`).
  fn footer(&self, ctx: &Ctx, props: &RunPermissionsProps, apply: Option<&Apply>, pending: bool) -> Element {
    let close = self.open.clone();
    let height = Dimension::Px(FOOTER_BUTTON_HEIGHT);
    let dismiss = ActionButton::new(ctx.t(if props.live { "common.cancel" } else { "common.close" }))
      .id("task-permissions-cancel")
      .compact()
      .in_form()
      .height(height)
      .on_click(move || close.set(false))
      .render(ctx);
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .justify(Justify::End)
      .align_items(Alignment::Center)
      .child(dismiss);
    if !props.live {
      return row.into();
    }
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
    row
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

/// "Running with  Builder · Edit files · 3 commands · Network off · MCP asks" (`UPDBO`), as the row
/// says it, without its link.
fn current_policy(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let text = policy_text(
    ctx,
    &props.step,
    (&props.permissions.invocation_policy, &props.permissions),
    Wording::Row,
  );
  policy_property(&text).id("task-permissions-current-policy").into()
}

/// When a change takes effect (`XpncF`, `aDPaV`), or, for an ended run, why it cannot be changed
/// (`uHLwL`).
fn takes_effect(ctx: &Ctx, props: &RunPermissionsProps) -> Element {
  let run = props.run_number.to_string();
  let (glyph, text) = if props.live {
    (
      Icon::Clock,
      ctx.t_args(
        "runs.permissions.popover.takesEffect",
        [
          ("agent", agent_of(&props.step)),
          ("step", &*props.step.name),
          ("run", run.as_str()),
        ],
      ),
    )
  } else {
    (
      Icon::Info,
      ctx.t_args("runs.permissions.popover.ended", [("agent", agent_of(&props.step))]),
    )
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Stack.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Start)
    .background(ExtraColor::Surface.role())
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Event, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::Detail.role()).flex(1.0))
    .into()
}
