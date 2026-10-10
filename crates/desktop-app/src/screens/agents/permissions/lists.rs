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
  node::{Element, dimension::Dimension, padding::Padding},
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
    form_section::{Label, Place, section, stacked_row},
    icon::Icon,
    list_entry,
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
    let mut folder_input = FieldInput::new("agent-folder", ctx.t("agents.folderPath"), self.folder.clone())
      .placeholder(ctx.t("agents.folderPath"))
      .mono();
    if let Some(error) = self.folder_error.get() {
      folder_input = folder_input.error(message_text(ctx, &error));
    }
    let folder_input = folder_input.render(ctx);
    let fields = props.fields.clone();
    let folder = self.folder.clone();
    let root = props.project.open.repo_root.clone();
    let folder_button =
      ActionButton::new("agent-add-folder", ButtonKind::Secondary, ctx.t("agents.addFolder")).icon(Icon::Folder);
    let folder_button = if folder_state.is_pending() {
      folder_button.loading()
    } else if fields.permissions.get().0.repository() != RepositoryAccess::Write {
      folder_button.disabled()
    } else {
      folder_button.on_click(move || add.run((root.clone(), folder.get())))
    };
    let mut entries = Vec::new();
    for entry in props.fields.permissions.get().0.writable_folders().entries() {
      let (entry, signal) = (entry.clone(), props.fields.permissions.clone());
      let text = entry.to_string();
      let remove_label = ctx.t("agents.remove");
      entries.push(list_entry::removable(
        ctx,
        &format!("agent-remove-folder-{}", entries.len()),
        &text,
        &remove_label,
        move || {
          signal.update(|p| {
            p.0
              .set_writable_folders(folders::without_folder(p.0.writable_folders(), &entry))
          });
        },
      ));
    }
    let mut folder_rows = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
    if !entries.is_empty() {
      folder_rows = folder_rows.child(list_entry::list_surface("agent-folder-entries").with_children(entries));
    }
    folder_rows = folder_rows
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Column::new().flex(1.0).child(folder_input))
          .child(folder_button),
      )
      .child(words::note(&ctx.t("agents.folderNote")));
    let commands = self.commands(ctx, &props);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .child(stacked_row(
        Label::new("agent-folders", ctx.t("agents.folders")).description(ctx.t("agents.foldersHint")),
        folder_rows,
        Place::Last,
      ))
      .child(commands)
  }
}
impl Lists {
  fn commands(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let fields = &props.fields;
    let provider = fields.connection(&props.listed).map(|c| c.provider());
    let mut allowed = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .child(command_option(ctx, fields, provider, PermissionOption::AllowedCommands));
    if let CommandPermission::Allowed(list) = fields.permissions.get().0.commands() {
      allowed = allowed.child(self.command_editor(ctx, props, list));
    }
    section(
      "agent-commands",
      &ctx.t("agents.commands"),
      None,
      vec![
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::new().top(SpacingSize::Md).bottom(SpacingSize::Xs))
          .spacing(crate::theme::ExtraSpacing::Inset.role())
          .child(command_option(
            ctx,
            fields,
            provider,
            PermissionOption::AskBeforeCommands,
          ))
          .child(allowed)
          .child(command_option(ctx, fields, provider, PermissionOption::AnyCommand))
          .into(),
      ],
    )
    .into()
  }
  fn command_editor(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    list: &orchester_core::permission::AllowedCommands,
  ) -> Element {
    let fields = &props.fields;
    let mut chips = Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .padding(Padding::symmetric(
        SpacingSize::Sm,
        crate::theme::ExtraSpacing::Snug.role(),
      ));
    for (index, command) in list.entries().iter().enumerate() {
      let (command, signal) = (command.clone(), fields.permissions.clone());
      chips = chips.child(
        crate::ui::chip::TagChip::new(format!("agent-command-{index}"), command.as_str())
          .mono()
          .removable(ctx.t("agents.remove"), move || {
            signal.update(|p| {
              if let CommandPermission::Allowed(list) = p.0.commands() {
                p.0
                  .set_commands(CommandPermission::Allowed(permissions::without_command(list, &command)));
              }
            });
          }),
      );
    }
    let input = FieldInput::new("agent-command", ctx.t("agents.command"), self.command.clone())
      .placeholder(ctx.t("agents.commandPlaceholder"))
      .mono()
      .render(ctx);
    let (value, typed, error) = (fields.permissions.clone(), self.command.clone(), self.error.clone());
    let add = ActionButton::new("agent-add-command", ButtonKind::Secondary, ctx.t("agents.addCommand"))
      .icon(Icon::Plus)
      .on_click(move || {
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
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Xl))
      .spacing(SpacingSize::Sm)
      .with_children(
        (!list.entries().is_empty()).then(|| list_entry::list_surface("agent-command-entries").child(chips)),
      )
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(Column::new().flex(1.0).child(input))
          .child(add),
      )
      .with_children(
        self
          .error
          .get()
          .map(|error| field_error("agent-command-error", &message_text(ctx, &error))),
      )
      .child(words::note(&ctx.t("agents.commandNote")))
      .into()
  }
}
fn folder_action(ctx: &mut Ctx) -> lurq::app::ctx::FutureAction<(PathBuf, String), Folder, Message> {
  ctx.future_action(|(root, typed): (PathBuf, String)| async move {
    background::run(move || folders::validate(&typed, &root).map(Folder)).await?
  })
}
