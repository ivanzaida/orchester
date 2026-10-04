//! The members of a shared project (10.13, 10.14 A, B): owners and the server's admins add people
//! by their exact username with a role and change roles in the rows; everyone sees the list and
//! leaves from their own row. Making someone an owner, making yourself a member, removing someone
//! and leaving ask first; making another owner a member acts at once.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_api::{ErrorCode, Member, MemberRole};
use orchester_core::{id::ProjectId, time::Timestamp};

use super::{Actions, MemberWork, Members, RoleChoice, SharingDialog, SharingTab, labels, local::CONTROL_HEIGHT};
use crate::{
  background,
  messages::Message,
  sharing::{
    feed::ServerFeed,
    project::{MemberChange, ProjectServer, state::ProjectSharing},
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, select_style, styled_input},
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Column widths of the member table (`OoSqW`, `HR8R2`, `QC9th`), the role select (`fJrKU`), and
/// the heights of a row, the heading row and a row's buttons (`h6oZVX`, `xeHey`, `AOEFC`).
const ROLE_WIDTH: f32 = 80.0;
const ADDED_WIDTH: f32 = 80.0;
const ACTIONS_WIDTH: f32 = 250.0;
const ROLE_SELECT_WIDTH: f32 = 120.0;
const ROW_HEIGHT: f32 = 52.0;
const HEADING_HEIGHT: f32 = 28.0;
const ROW_BUTTON_HEIGHT: f32 = 28.0;

impl SharingTab {
  /// *Members* (`p0sWX`).
  pub(super) fn members(&self, ctx: &mut Ctx, sharing: &ProjectSharing, actions: &Actions) -> Element {
    let members = self.load_members(ctx, sharing);
    let manage = sharing.link.role == Some(MemberRole::Owner) || sharing.admin;
    let key = sharing.link.key.as_str();
    let server = sharing.server_name();
    let count = members
      .as_ref()
      .and_then(|members| members.as_ref().ok())
      .map(|members| members.0.len());
    let mut heading = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(&ctx.t("sharing.members.heading")).variant(ExtraTypography::SectionHeading.role()));
    if let Some(count) = count {
      heading = heading.child(Text::new(&count.to_string()).variant(ExtraTypography::NavCount.role()));
    }
    let description = if manage {
      ctx.t_args("sharing.members.detail", [("key", key), ("server", server)])
    } else {
      ctx.t_args("sharing.members.memberNote", [("key", key), ("server", server)])
    };
    let mut column = Column::new()
      .id("sharing-members")
      .width(Dimension::Pct(100.0))
      .padding_top(ExtraSpacing::Form.role())
      .padding_bottom(ExtraSpacing::Divider.role())
      .spacing(ExtraSpacing::Inline.role())
      .overflow_visible()
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(heading)
      .child(
        Text::new(&description)
          .variant(ExtraTypography::Detail.role())
          .width(Dimension::Pct(100.0)),
      );
    if manage {
      column = column.child(self.add_form(ctx, actions));
    }
    let error = actions
      .member
      .state()
      .get()
      .error
      .filter(|_| actions.member.state().get().is_rejected() && self.dialog.get() == SharingDialog::Closed);
    if let Some(error) = error {
      column = column.child(
        Column::new()
          .id("sharing-member-error")
          .child(field_error(&message_text(ctx, &self.named_error(sharing, &error)))),
      );
    }
    let table: Element = match members {
      None => Text::new(&ctx.t("sharing.members.loading"))
        .variant(TypographyStyle::Caption)
        .into(),
      Some(Err(error)) => field_error(&message_text(ctx, &error)),
      Some(Ok(members)) => self.table(ctx, sharing, &members, (manage, actions)),
    };
    column.child(table).into()
  }

  /// The username, its role and *Add member* (`vyYnO`).
  fn add_form(&self, ctx: &mut Ctx, actions: &Actions) -> Element {
    let username = self.username.get().trim().to_owned();
    let pending = actions.member.state().get().is_pending();
    let (action, role) = (actions.member.clone(), self.role.clone());
    let add = (!username.is_empty() && !pending).then(|| {
      Arc::new(move || {
        action.run(MemberWork::Add {
          username: username.clone(),
          role: role.get_untracked().0,
        });
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let roles = [MemberRole::Member, MemberRole::Owner]
      .into_iter()
      .map(|role| (RoleChoice(role), ctx.t(labels::role_key(role))));
    let select = Element::from(
      Select::new(self.role.clone())
        .options(roles)
        .width(Dimension::Px(ROLE_SELECT_WIDTH))
        .height(Dimension::Px(CONTROL_HEIGHT))
        .style(select_style(ctx, CONTROL_HEIGHT)),
    )
    .id("sharing-member-role");
    let button = ActionButton::new(ctx.t("sharing.members.add"))
      .icon(Icon::UserPlus)
      .id("sharing-member-add")
      .in_form()
      .height(Dimension::Px(CONTROL_HEIGHT));
    let button = match add {
      Some(add) => button.on_click(move || add()),
      None => button.on_click(|| {}).enabled(false),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Xs)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(username_field(ctx, &self.username))
      .child(select)
      .child(button.render(ctx))
      .into()
  }

  /// A refusal of *Add member* names the username the person typed.
  fn named_error(&self, sharing: &ProjectSharing, error: &Message) -> Message {
    let username = self.username.get_untracked().trim().to_owned();
    match error.key {
      "sharing.members.noSuchUser" => Message::new("sharing.members.noSuchUserNamed")
        .with("server", sharing.server_name().to_owned())
        .with("username", username),
      "sharing.members.alreadyMember" => Message::new("sharing.members.alreadyMemberNamed")
        .with("key", sharing.link.key.as_str().to_owned())
        .with("username", username),
      _ => error.clone(),
    }
  }

  fn table(
    &self,
    ctx: &Ctx,
    sharing: &ProjectSharing,
    members: &Members,
    (manage, actions): (bool, &Actions),
  ) -> Element {
    let heading = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADING_HEIGHT))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(column_name(ctx, "sharing.members.columns.name").flex(1.0))
      .child(column_name(ctx, "sharing.members.columns.role").width(Dimension::Px(ROLE_WIDTH)))
      .child(column_name(ctx, "sharing.members.columns.added").width(Dimension::Px(ADDED_WIDTH)))
      .child(Row::new().width(Dimension::Px(ACTIONS_WIDTH)));
    let pending = actions.member.state().get().is_pending();
    Column::new()
      .id("sharing-member-table")
      .width(Dimension::Pct(100.0))
      .child(heading)
      .with_children(members.0.iter().map(|member| {
        let buttons = self.row_actions(ctx, sharing, member, (manage, pending, actions));
        member_row(ctx, sharing, member, buttons)
      }))
      .into()
  }

  /// A row's actions: on an owner's row *Make member*, on a member's *Make owner*, and *Remove*, for
  /// owners and admins; on the person's own row *Leave*.
  fn row_actions(
    &self,
    ctx: &Ctx,
    sharing: &ProjectSharing,
    member: &Member,
    (manage, pending, actions): (bool, bool, &Actions),
  ) -> Vec<Element> {
    let mine = Some(member.user.id) == sharing.me;
    let slug = member.user.id.to_string();
    let mut buttons = Vec::new();
    if manage {
      let (dialog, action, shown) = (self.dialog.clone(), actions.member.clone(), member.clone());
      let button = match (member.role, mine) {
        (MemberRole::Owner, true) => row_button(ctx, ("sharing.members.makeMember", Icon::User), &slug, "make-member")
          .on_click(move || dialog.set(SharingDialog::MakeSelfMember)),
        (MemberRole::Owner, false) => row_button(ctx, ("sharing.members.makeMember", Icon::User), &slug, "make-member")
          .on_click(move || {
            action.run(MemberWork::Change(MemberChange::Role(
              shown.user.id,
              MemberRole::Member,
            )))
          }),
        (MemberRole::Member, _) => row_button(ctx, ("sharing.members.makeOwner", Icon::Crown), &slug, "make-owner")
          .on_click(move || dialog.set(SharingDialog::MakeOwner(shown.clone()))),
      };
      buttons.push(button.enabled(!pending).render(ctx));
    }
    let dialog = self.dialog.clone();
    if mine {
      buttons.push(
        row_button(ctx, ("sharing.members.leave", Icon::LogOut), &slug, "leave")
          .on_click(move || dialog.set(SharingDialog::Leave))
          .enabled(!pending)
          .render(ctx),
      );
    } else if manage {
      let shown = member.clone();
      buttons.push(
        row_button(ctx, ("sharing.members.remove", Icon::UserMinus), &slug, "remove")
          .on_click(move || dialog.set(SharingDialog::Remove(shown.clone())))
          .enabled(!pending)
          .render(ctx),
      );
    }
    buttons
  }

  /// The project's members from its server, read again after each change; `None` while read.
  pub(super) fn load_members(&self, ctx: &mut Ctx, sharing: &ProjectSharing) -> Option<Result<Members, Message>> {
    let servers = ServerFeed::of(ctx).servers;
    let root = ctx.props::<super::SharingProps>().open.repo_root.clone();
    let deps = (root, sharing.link.project, self.revision.get());
    let load = ctx.future(deps, move |(root, ..): (PathBuf, ProjectId, u64)| {
      let servers = servers.clone();
      async move {
        let server = background::run({
          let (servers, root) = (servers.clone(), root.clone());
          move || ProjectServer::open(&servers, &root)
        })
        .await?
        .map_err(|error| super::error_message(&error, None))?
        .ok_or_else(|| Message::new("sharing.errors.notShared"))?;
        server
          .members()
          .await
          .map(|members| Members(members.into()))
          .map_err(|error| match error.code() {
            // The server no longer has the person as a member: the project is read again, so the
            // list says so at once.
            Some(ErrorCode::NotFound) => {
              servers.sync_now();
              Message::new("sharing.errors.notMember")
            }
            _ => error.message(&server.entry().address),
          })
      }
    });
    let state = load.state().get();
    let pending = state.is_pending();
    match (state.data, state.error) {
      (_, Some(error)) if !pending => Some(Err(error)),
      (Some(members), _) => Some(Ok(members)),
      _ => None,
    }
  }
}

fn username_field(ctx: &Ctx, value: &Signal<String>) -> Element {
  let input = ctx.theme().form().input.clone();
  Row::new()
    .flex(1.0)
    .height(Dimension::Px(CONTROL_HEIGHT))
    .padding_custom(input.padding)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(input.background)
    .border(Border::inside(BorderSize::Sm, input.border))
    .rounded(input.radius)
    .child(
      styled_input(ctx, value.clone(), TypographyStyle::FieldLabel)
        .id("sharing-member-username")
        .single_line()
        .flex(1.0)
        .placeholder(&ctx.t("sharing.members.username")),
    )
    .child(icon(Icon::AtSign, IconSize::Dismiss, PaletteColor::TextSecondary))
    .into()
}

fn column_name(ctx: &Ctx, key: &str) -> Row {
  Row::new().child(
    Text::new(&ctx.t(key).to_uppercase())
      .variant(ExtraTypography::ColumnHeading.role())
      .nowrap(),
  )
}

fn row_button(ctx: &Ctx, (key, glyph): (&str, Icon), member: &str, action: &str) -> ActionButton {
  ActionButton::new(ctx.t(key))
    .icon(glyph)
    .id(Arc::<str>::from(format!("sharing-member-{action}-{member}")))
    .in_form()
    .height(Dimension::Px(ROW_BUTTON_HEIGHT))
}

/// A member's row (`h6oZVX`): name with *you* on the person's own row, role, when added, actions.
fn member_row(ctx: &Ctx, sharing: &ProjectSharing, member: &Member, buttons: Vec<Element>) -> Element {
  let mine = Some(member.user.id) == sharing.me;
  let mut name = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&member.user.display_name)
        .variant(ExtraTypography::FileName.role())
        .nowrap(),
    );
  if mine {
    name = name.child(Text::new(&ctx.t("sharing.members.you")).variant(ExtraTypography::Footnote.role()));
  }
  let role_color = match member.role {
    MemberRole::Owner => PaletteColor::TextPrimary,
    MemberRole::Member => PaletteColor::TextSecondary,
  };
  Row::new()
    .id(Arc::<str>::from(format!("sharing-member-{}", member.user.id)))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ROW_HEIGHT))
    .padding_horizontal(ExtraSpacing::Stack.role())
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .overflow_visible()
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(name)
    .child(
      Row::new().width(Dimension::Px(ROLE_WIDTH)).child(
        Text::new(&ctx.t(labels::role_key(member.role)))
          .variant(TypographyStyle::Caption)
          .color(role_color),
      ),
    )
    .child(
      Row::new().width(Dimension::Px(ADDED_WIDTH)).child(
        Text::new(&labels::day(ctx, Timestamp::from_unix_millis(member.added_at)))
          .variant(ExtraTypography::MonoValue.role())
          .color(PaletteColor::TextSecondary),
      ),
    )
    .child(
      Row::new()
        .width(Dimension::Px(ACTIONS_WIDTH))
        .spacing(SpacingSize::Sm)
        .justify(lurq::layout::layout_kind::Justify::End)
        .overflow_visible()
        .with_children(buttons),
    )
    .into()
}
