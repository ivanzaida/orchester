//! Command and folder entries, validated by workspace before being added.
use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::permission::{CommandPermission, RepositoryAccess, WritableFolder, capability::PermissionOption};
use orchester_workspace::{
  agents::{folders, permissions},
  background,
  messages::Message,
};

use super::{Props, command_option};
use crate::{
  screens::agents::words,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    form_section::section,
    message::message_text,
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Folder(WritableFolder);
impl DevtoolsInspectable for Folder {}
pub struct Lists {
  command: Signal<String>,
  folder: Signal<String>,
  error: Signal<Option<Message>>,
  folder_error: Signal<Option<Message>>,
}
impl Component for Lists {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let fields = ctx.props::<Props>().fields.clone();
    let component = Self {
      command: ctx.signal(String::new()),
      folder: ctx.signal(String::new()),
      error: ctx.signal(None),
      folder_error: ctx.signal(None),
    };
    let add = folder_action(ctx);
    let typed = component.folder.clone();
    let error = component.folder_error.clone();
    ctx.watch(&add.state(), move |state| {
      if state.is_pending() {
        error.set(None);
      } else if let Some(Folder(folder)) = state.data.clone().filter(|_| state.is_fulfilled()) {
        fields.permissions.update(
          |p| match folders::with_validated_folder(p.0.writable_folders(), folder) {
            Ok(folders) => {
              p.0.set_writable_folders(folders);
              typed.set(String::new());
            }
            Err(reason) => error.set(Some(reason)),
          },
        );
      } else if state.is_rejected() {
        error.set(state.error.clone());
      }
    });
    component
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let add = folder_action(ctx);
    let folder_state = add.state().get();
    let folder_input = FieldInput::new("agent-folder", ctx.t("agents.folderPath"), self.folder.clone()).render(ctx);
    let fields = props.fields.clone();
    let folder = self.folder.clone();
    let root = props.project.open.repo_root.clone();
    let folder_button = ActionButton::new("agent-add-folder", ButtonKind::Secondary, ctx.t("agents.addFolder"));
    let folder_button = if folder_state.is_pending() {
      folder_button.loading()
    } else if fields.permissions.get().0.repository() != RepositoryAccess::Write {
      folder_button.disabled()
    } else {
      folder_button.on_click(move || add.run((root.clone(), folder.get())))
    };
    let mut folder_rows: Vec<Element> = Vec::new();
    for entry in props.fields.permissions.get().0.writable_folders().entries() {
      let (entry, signal) = (entry.clone(), props.fields.permissions.clone());
      let text = entry.to_string();
      folder_rows.push(entry_row(
        ctx,
        &text,
        format!("agent-remove-folder-{}", folder_rows.len()),
        move || {
          signal.update(|p| {
            p.0
              .set_writable_folders(folders::without_folder(p.0.writable_folders(), &entry))
          });
        },
      ));
    }
    folder_rows.push(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Column::new().flex(1.0).child(folder_input))
        .child(folder_button)
        .into(),
    );
    folder_rows.push(words::note(&ctx.t("agents.folderNote")).into());
    if let Some(error) = self.folder_error.get() {
      folder_rows.push(field_error("agent-folder-error", &message_text(ctx, &error)).into());
    }
    let commands = self.commands(ctx, &props);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .child(section("agent-folders", &ctx.t("agents.folders"), None, folder_rows))
      .child(commands)
  }
}
impl Lists {
  fn commands(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let fields = &props.fields;
    let provider = fields.connection(&props.listed).map(|c| c.provider());
    let mut rows = vec![
      command_option(ctx, fields, provider, PermissionOption::AskBeforeCommands),
      command_option(ctx, fields, provider, PermissionOption::AllowedCommands),
    ];
    if let CommandPermission::Allowed(list) = fields.permissions.get().0.commands() {
      for command in list.entries() {
        let (command, signal) = (command.clone(), fields.permissions.clone());
        rows.push(
          crate::ui::chip::TagChip::new(format!("agent-command-{}", rows.len()), command.as_str())
            .mono()
            .removable(ctx.t("agents.remove"), {
              let command = command.clone();
              move || {
                signal.update(|p| {
                  if let CommandPermission::Allowed(list) = p.0.commands() {
                    p.0
                      .set_commands(CommandPermission::Allowed(permissions::without_command(list, &command)));
                  }
                })
              }
            })
            .into(),
        );
      }
      let input = FieldInput::new("agent-command", ctx.t("agents.command"), self.command.clone())
        .placeholder(ctx.t("agents.commandPlaceholder"))
        .render(ctx);
      let (value, typed, error) = (fields.permissions.clone(), self.command.clone(), self.error.clone());
      let add =
        ActionButton::new("agent-add-command", ButtonKind::Secondary, ctx.t("agents.addCommand")).on_click(move || {
          if let CommandPermission::Allowed(list) = value.get().0.commands() {
            match permissions::with_command(list, &typed.get()) {
              Ok(list) => {
                value.update(|p| p.0.set_commands(CommandPermission::Allowed(list)));
                typed.set(String::new());
                error.set(None);
              }
              Err(reason) => error.set(Some(reason)),
            }
          }
        });
      rows.push(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Column::new().flex(1.0).child(input))
          .child(add)
          .into(),
      );
      if let Some(error) = self.error.get() {
        rows.push(field_error("agent-command-error", &message_text(ctx, &error)).into());
      }
      rows.push(words::note(&ctx.t("agents.commandNote")).into());
    }
    rows.push(command_option(ctx, fields, provider, PermissionOption::AnyCommand));
    section(
      "agent-commands",
      &ctx.t("agents.commands"),
      None,
      vec![Column::new().spacing(SpacingSize::Sm).with_children(rows).into()],
    )
    .into()
  }
}
fn folder_action(ctx: &mut Ctx) -> lurq::app::ctx::FutureAction<(PathBuf, String), Folder, Message> {
  ctx.future_action(|(root, typed): (PathBuf, String)| async move {
    background::run(move || folders::validate(&typed, &root).map(Folder)).await?
  })
}
fn entry_row(ctx: &Ctx, text: &str, id: String, remove: impl Fn() + Send + Sync + 'static) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Column::new().flex(1.0).child(words::note(text)))
    .child(ActionButton::new(id, ButtonKind::Ghost, ctx.t("agents.remove")).on_click(remove))
    .into()
}
