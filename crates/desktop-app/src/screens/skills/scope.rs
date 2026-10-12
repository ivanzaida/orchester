//! Grouped scope menu LKNHu in DKP6I, using the app's field and menu surfaces.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::{instructions::form::Picked, skills::editor::ScopeOption};

use super::{PageProps, words};
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    menu::{self},
    pressable::{Focus, inert, pressable},
  },
};
const WIDTH: f32 = 380.0;
const MENU_HEIGHT: f32 = 700.0;
#[derive(Clone)]
pub struct Props {
  pub page: PageProps,
  pub value: Signal<Picked<InstructionScope>>,
  pub locked: bool,
}
impl PartialEq for Props {
  fn eq(&self, o: &Self) -> bool {
    self.page == o.page && self.value.id() == o.value.id() && self.locked == o.locked
  }
}
impl DevtoolsInspectable for Props {}
pub struct ScopePicker {
  open: Signal<bool>,
  anchor: ElementRef,
}
impl Component for ScopePicker {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<Props>().clone();
    let open = self.open.clone();
    let locked = p.locked;
    let trigger = Focus::Form
      .apply(pressable(move || {
        if !locked {
          open.update(|o| *o = !*o);
        }
      }))
      .id("skill-scope")
      .ref_element(self.anchor.clone())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .rounded(RadiusSize::Md)
      .background(PaletteColor::SurfaceInput)
      .border_inside(
        BorderSize::Sm,
        if self.open.get() {
          PaletteColor::BorderFocus
        } else {
          PaletteColor::Border
        },
      )
      .describe("a11y_name", ctx.t("skills.scope"))
      .describe("locked", if locked { "true" } else { "false" })
      .child(
        Text::new(&words::scope(ctx, &p.page.data, &p.value.get().0))
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .nowrap(),
      )
      .child(icon(
        if self.open.get() {
          Icon::ChevronUp
        } else {
          Icon::ChevronDown
        },
        IconSize::Md,
        PaletteColor::TextMuted,
      ));
    let trigger = if locked {
      trigger.focusable(false).opacity(crate::theme::DISABLED_OPACITY)
    } else {
      trigger
    };
    let mut frame = Column::new().width(Dimension::Pct(100.0)).child(trigger);
    if self.open.get() && !locked {
      let panel = self.options(ctx, &p);
      frame = frame.child(menu::menu(self.anchor.clone(), self.open.clone(), panel));
    }
    frame
  }
}
impl ScopePicker {
  fn options(&self, ctx: &mut Ctx, p: &Props) -> Element {
    let mut list = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role());
    for (key, kind) in [
      ("skills.scopeMenu.project", 0),
      ("skills.scopeMenu.roles", 1),
      ("skills.scopeMenu.pipelines", 2),
    ] {
      let options = p
        .page
        .data
        .scopes
        .iter()
        .filter(|o| {
          matches!(
            (&o.scope, kind),
            (InstructionScope::Project, 0)
              | (InstructionScope::AgentRole { .. }, 1)
              | (InstructionScope::Pipeline { .. }, 2)
          )
        })
        .collect::<Vec<_>>();
      if !options.is_empty() {
        list = list.child(heading(&ctx.t(key)));
      }
      for option in options {
        list = list.child(self.option(ctx, p, option));
      }
    }
    for pipeline in p
      .page
      .data
      .scopes
      .iter()
      .filter(|o| matches!(o.scope, InstructionScope::Pipeline { .. }))
    {
      let InstructionScope::Pipeline { pipeline: id } = pipeline.scope else {
        continue;
      };
      list = list.child(heading(&ctx.t_args(
        "skills.scopeMenu.steps",
        [("pipeline", pipeline.pipeline.as_deref().unwrap_or_default())],
      )));
      for option in p
        .page
        .data
        .scopes
        .iter()
        .filter(|o| matches!(o.scope,InstructionScope::Step{pipeline,..} if pipeline==id))
      {
        list = list.child(self.option(ctx, p, option));
      }
    }
    list = list.child(
      Text::new(&ctx.t("skills.toolStep"))
        .variant(ExtraTypography::Note.role())
        .padding(SpacingSize::Sm),
    );
    menu::menu_panel("skill-scope-menu")
      .width(Dimension::Px(WIDTH))
      .padding(ExtraSpacing::Snug.role())
      .rounded(RadiusSize::Lg)
      .child(
        ScrollVertical::new(list)
          .width(Dimension::Pct(100.0))
          .max_height(Dimension::Px(MENU_HEIGHT)),
      )
      .into()
  }
  fn option(&self, ctx: &mut Ctx, p: &Props, o: &ScopeOption) -> Element {
    let (label, detail, glyph) = option_words(ctx, p, o);
    let index = p
      .page
      .data
      .scopes
      .iter()
      .position(|item| item.scope == o.scope)
      .expect("option is from these scopes");
    let id = format!("skill-scope-option-{index}");
    let value = p.value.clone();
    let scope = o.scope.clone();
    let open = self.open.clone();
    let item = if o.available {
      Focus::Form.apply(pressable(move || {
        open.set(false);
        value.set(Picked(scope.clone()));
      }))
    } else {
      inert(Button::empty()).opacity(crate::theme::DISABLED_OPACITY)
    };
    let chosen = p.value.get().0 == o.scope;
    let item = item
      .id(id)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .describe("a11y_name", label.clone())
      .describe("state", if o.available { "enabled" } else { "unavailable" })
      .rounded(RadiusSize::Sm)
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .hovered_style(Style::new().background(crate::theme::ExtraColor::Hover.role()))
      .child(icon(
        if chosen { Icon::Check } else { glyph },
        IconSize::Md,
        PaletteColor::TextSecondary,
      ))
      .child(
        Text::new(&label)
          .variant(TypographyStyle::Description)
          .flex(1.0)
          .nowrap(),
      )
      .child(
        Text::new(&if o.available {
          detail
        } else {
          ctx.t("skills.scopeMenu.tool")
        })
        .variant(TypographyStyle::Caption)
        .nowrap(),
      );
    if chosen {
      item.background(crate::theme::ExtraColor::Hover.role()).into()
    } else {
      item.into()
    }
  }
}
fn heading(label: &str) -> Text {
  Text::new(label).variant(ExtraTypography::ColumnHeading.role()).padding(
    Padding::new()
      .top(SpacingSize::Sm)
      .left(SpacingSize::Sm)
      .bottom(SpacingSize::Xs),
  )
}
fn option_words(ctx: &Ctx, p: &Props, o: &ScopeOption) -> (std::sync::Arc<str>, std::sync::Arc<str>, Icon) {
  match &o.scope {
    InstructionScope::Project => (
      p.page.project.open.project.name().as_str().into(),
      ctx.t("skills.scopeMenu.everyAgent"),
      Icon::FolderGit2,
    ),
    InstructionScope::AgentRole { role } => (role.as_str().into(), ctx.t("skills.scopeMenu.profileAgents"), Icon::Bot),
    InstructionScope::Pipeline { .. } => (
      o.pipeline.clone().unwrap_or_else(|| ctx.t("skills.scopes.removed")),
      o.published.map_or_else(
        || ctx.t("skills.scopeMenu.agentSteps"),
        |v| ctx.t_args("skills.scopeMenu.pipelineVersion", [("version", v.get().to_string())]),
      ),
      Icon::Workflow,
    ),
    InstructionScope::Step { .. } => (
      o.step.clone().unwrap_or_else(|| ctx.t("skills.scopes.removed")),
      o.agent.clone().unwrap_or_else(|| ctx.t("skills.none")),
      Icon::GitCommitHorizontal,
    ),
  }
}
