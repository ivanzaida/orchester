//! The Stack group (`Stack` `E0LdDR`: its name, which agents and suites name it by; a stack keeps the name it was made
//! with) and the Services group (`Services` `x4S3LY`): its count and *In start order* at its heading, then a row per
//! service a start brings up (`postgres` `O9SuB`): the step it starts on in a 20 round mark, its name in Geist Mono
//! with its driver's chip, and what brought it — *Needed by …* with *from needs* at the row's end, or *Chosen* (with
//! the services that start with it) and a × that removes it — then *Add a service* with that its needs come along
//! (`Add` `z2pU4`). Services that do not resolve show as chosen, with why under the group.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor},
  },
  components::{Row, SelectOption, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::agent_testing::stack_editor::order::Member;

use super::{
  super::{
    super::kit::{Group, SettingRow},
    editor::fields::{field, input, with_error},
    words,
  },
  Commit, StackEditor, View,
};
use crate::{
  theme::{ExtraRadius, ExtraTypography, ProjectSettingsText, ServicesText},
  ui::{
    chip::TagChip,
    field::{SelectField, field_error},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
  },
};

/// A service's step mark (`Step` `a0o7Uf`).
const STEP_SIZE: f32 = 20.0;
/// *Add a service* (`Select` `J5Hqf`).
const ADD_WIDTH: f32 = 200.0;
/// What joins the names of a hint: a sign, not words.
const LIST: &str = ", ";

/// The Stack group: the name, typed for a new stack, shown locked for one that keeps it.
pub(super) fn stack_group(ctx: &mut Ctx, editor: &StackEditor, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.stack.name");
  let control = input(("settings-stack-name", label.clone()), &editor.name, commit);
  let control = if view.base().is_some() {
    control.locked()
  } else {
    control
  };
  let error = view.problems().and_then(|problems| problems.name.as_ref());
  let control = with_error(ctx, control, error).render(ctx);
  let row = field(
    "settings-stack-name",
    label,
    ctx.t("project_settings.services.stack.nameHint"),
  )
  .render(ctx, control);
  Group::new("settings-stack-group", ctx.t("project_settings.services.stack.title"))
    .row(row)
    .into()
}

/// The Services group.
pub(super) fn services_group(ctx: &mut Ctx, editor: &StackEditor, view: &View, commit: &Commit) -> Element {
  const ID: &str = "settings-stack-services";
  let chosen = editor.services.get().0;
  let (rows, problem) = match &view.members {
    Some(Ok(members)) => (
      members
        .iter()
        .map(|member| member_row(ctx, editor, member, commit))
        .collect::<Vec<_>>(),
      None,
    ),
    resolved => (
      chosen
        .iter()
        .map(|name| chosen_row(ctx, editor, (name, view), commit))
        .collect(),
      resolved.as_ref().and_then(|members| members.as_ref().err()).cloned(),
    ),
  };
  let count = rows.len();
  let error = view
    .problems()
    .and_then(|problems| problems.services.clone())
    .or(problem);
  let group = Group::new(ID, ctx.t("project_settings.services.services"))
    .beside(
      Text::new(&count.to_string())
        .id(format!("{ID}-count"))
        .variant(ProjectSettingsText::Hint.role()),
    )
    .end(
      Text::new(&ctx.t("project_settings.services.stack.inStartOrder"))
        .id(format!("{ID}-note"))
        .variant(ExtraTypography::Note.role()),
    )
    .rows(rows)
    .row(add_row(ctx, editor, view));
  match error {
    Some(error) => group
      .after(field_error("settings-stack-services-error", &message_text(ctx, &error)))
      .into(),
    None => group.into(),
  }
}

/// A service a start brings up, with its step and what brought it.
fn member_row(ctx: &mut Ctx, editor: &StackEditor, member: &Member, commit: &Commit) -> Element {
  let id = format!("settings-stack-service-{}", member.name);
  let hint = if member.chosen {
    if member.together.is_empty() {
      ctx.t("project_settings.services.stack.chosen")
    } else {
      ctx.t_args(
        "project_settings.services.stack.chosenWith",
        [("names", names(&member.together))],
      )
    }
  } else {
    ctx.t_args(
      "project_settings.services.stack.neededBy",
      [("names", names(&member.needed_by))],
    )
  };
  let end = if member.chosen {
    remove(ctx, editor, (&id, &member.name), commit)
  } else {
    Text::new(&ctx.t("project_settings.services.stack.fromNeeds"))
      .id(format!("{id}-from-needs"))
      .variant(ProjectSettingsText::Hint.role())
      .nowrap()
      .into()
  };
  SettingRow::new(id.clone(), member.name.clone())
    .item()
    .mono_label()
    .lead(step_mark(&id, Some(member.step)))
    .beside(TagChip::new(format!("{id}-driver"), words::driver(ctx, member.driver)))
    .hint(hint)
    .control(end)
    .render(ctx)
}

/// A chosen service while the stack's services do not resolve: its name, *Chosen* and its ×.
fn chosen_row(ctx: &mut Ctx, editor: &StackEditor, (name, view): (&str, &View), commit: &Commit) -> Element {
  let id = format!("settings-stack-service-{name}");
  let driver = view
    .stored
    .services
    .iter()
    .find(|entry| &*entry.name == name)
    .map(|entry| TagChip::new(format!("{id}-driver"), words::driver(ctx, entry.body.ui_driver())));
  let row = SettingRow::new(id.clone(), name.to_owned())
    .item()
    .mono_label()
    .lead(step_mark(&id, None))
    .hint(ctx.t("project_settings.services.stack.chosen"))
    .control(remove(ctx, editor, (&id, name), commit));
  match driver {
    Some(driver) => row.beside(driver),
    None => row,
  }
  .render(ctx)
}

/// The step a service starts on in its round mark; blank while the services do not resolve.
fn step_mark(id: &str, step: Option<usize>) -> Element {
  Row::new()
    .id(format!("{id}-step"))
    .width(Dimension::Px(STEP_SIZE))
    .height(Dimension::Px(STEP_SIZE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .with_children(step.map(|step| Text::new(&step.to_string()).variant(ServicesText::StepNumber.role())))
    .into()
}

/// The × that takes a chosen service out of the stack and saves.
fn remove(ctx: &mut Ctx, editor: &StackEditor, (id, name): (&str, &str), commit: &Commit) -> Element {
  let (services, commit, removed) = (editor.services.clone(), commit.clone(), name.to_owned());
  IconButton::new(
    format!("{id}-remove"),
    Icon::X,
    ctx.t_args("project_settings.services.editor.removeNeed", [("name", name)]),
  )
  .small()
  .on_click(move || {
    services.update(|services| services.0.retain(|service| *service != removed));
    commit();
  })
  .render(ctx)
}

/// *Add a service*: the project's services not chosen yet; choosing one adds it and saves (the page follows
/// [`StackEditor::adding`]).
fn add_row(ctx: &mut Ctx, editor: &StackEditor, view: &View) -> Element {
  let chosen = editor.services.get().0;
  let offered: Vec<SelectOption<String>> = view
    .stored
    .services
    .iter()
    .map(|entry| entry.name.to_string())
    .filter(|name| !chosen.contains(name))
    .map(|name| SelectOption::new(name.clone(), name))
    .collect();
  let placeholder = if offered.is_empty() {
    "project_settings.services.stack.noMoreServices"
  } else {
    "project_settings.services.editor.addNeed"
  };
  let select = SelectField::new(
    "settings-stack-add-service",
    ctx.t("project_settings.services.editor.addNeed"),
    editor.adding.clone(),
    offered.clone(),
  )
  .placeholder(ctx.t(placeholder))
  .width(Dimension::Px(ADD_WIDTH));
  let select = if offered.is_empty() { select.locked() } else { select }.render(ctx);
  Row::new()
    .id("settings-stack-add-row")
    .width(Dimension::Pct(100.0))
    .padding(lurq::node::padding::Padding::symmetric(
      crate::theme::ProjectSettingsSpacing::RowX.role(),
      crate::theme::ExtraSpacing::Loose.role(),
    ))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(select)
    .child(
      Text::new(&ctx.t("project_settings.services.stack.addNote"))
        .id("settings-stack-add-note")
        .variant(ProjectSettingsText::Hint.role()),
    )
    .into()
}

fn names(names: &[std::sync::Arc<str>]) -> String {
  names.iter().map(|name| &**name).collect::<Vec<_>>().join(LIST)
}
