//! The parts of a local project's Sharing around the link: the project as local with *Share…* (`04 Sharing · local`
//! `bSZdW`) or the sign-in it needs (`g3c5AM`), where projects are made, and what linking sends.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sharing::ServerEntry;

use super::Props;
use crate::{
  navigation::Navigation,
  routes::{JoinWay, Route},
  screens::{
    cloud_sign_in::{Place, SignInOffer},
    project_settings::{
      kit::{Group, SettingRow, value},
      page,
    },
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText, ProjectSettingsSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// The label column of *What linking sends* (`Shared with members` in `Fy5KW`).
const SENDS_LABEL: f32 = 180.0;

/// The project as local (`Local project` `bIi5a`: its tasks only on this computer, with its key and how many) and
/// *Share on a team server*: with *Share…*, which opens the page that links it, or `signing_in`: what sharing needs
/// while nobody is signed in and no server is joined, with *Use a code* and *Sign in*, which opens in place under the
/// row (`g3c5AM`, `i5lddT`).
pub(super) fn local(
  ctx: &mut Ctx,
  props: &Props,
  share: impl Fn() + Send + Sync + 'static,
  signing_in: Option<&Signal<bool>>,
) -> Element {
  let name = props.project.open.project.name().as_str();
  let key = props.project.open.project.key().as_str();
  let tasks = ctx.t_args(
    "sharing.link.localTasks",
    [("key", key.to_owned()), ("count", props.facts.tasks.to_string())],
  );
  let only_you = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Monitor, IconSize::Base, PaletteColor::TextMuted))
    .child(value("sharing-local", &ctx.t("sharing.share.onlyYou")));
  let local = SettingRow::new("sharing-task-list", ctx.t("sharing.share.localProject"))
    .hint(tasks)
    .info(ctx.t_args("sharing.link.taskListDescription", [("project", name), ("key", key)]))
    .control(only_you)
    .render(ctx);
  let share = match signing_in {
    Some(open) => offer_row(ctx, open),
    None => SettingRow::new("sharing-share", ctx.t("sharing.share.onServer"))
      .hint(ctx.t("sharing.share.onServerHint"))
      .info(ctx.t("sharing.link.hint"))
      .control(
        ActionButton::new("sharing-share-open", ButtonKind::Secondary, ctx.t("sharing.share.open"))
          .icon(Icon::CloudUpload)
          .in_form()
          .on_click(share),
      )
      .render(ctx),
  };
  Group::new("project-settings-sharing-task-list", ctx.t("project_settings.sharing"))
    .row(local)
    .row(share)
    .into()
}

/// *Share on a team server* while it needs a sign-in, and the sign-in under it once *Sign in* opened it.
fn offer_row(ctx: &mut Ctx, open: &Signal<bool>) -> Element {
  let opened = open.get();
  let navigation = Navigation::of(ctx);
  let code = ActionButton::new("sharing-use-code", ButtonKind::Ghost, ctx.t("sharing.share.useCode"))
    .in_form()
    .opens(navigation.to(Route::JoinTeam(JoinWay::Code)));
  let flip = open.clone();
  let sign_in = ActionButton::new("sharing-sign-in", ButtonKind::Secondary, ctx.t("sharing.share.signIn"))
    .icon(Icon::LogIn)
    .held(opened)
    .in_form()
    .on_click(move || flip.update(|open| *open = !*open));
  let row = SettingRow::new("sharing-share", ctx.t("sharing.share.onServer"))
    .hint(ctx.t("sharing.share.signInHint"))
    .control(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(code)
        .child(sign_in),
    )
    .render(ctx);
  let form = opened.then(|| {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::horizontal(ProjectSettingsSpacing::RowX.role()).bottom(ProjectSettingsSpacing::RowX.role()))
      .child(ctx.mount_keyed::<SignInOffer>("sharing-sign-in-form", Place::Under))
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children(form)
    .into()
}

/// *No project for this yet?* Projects are made in the server admin.
pub(super) fn admin_note(ctx: &Ctx, entry: Option<&ServerEntry>) -> Element {
  let words = entry.map_or_else(
    || ctx.t("sharing.link.noProjectYetAny").to_string(),
    |entry| {
      ctx
        .t_args(
          "sharing.link.noProjectYet",
          [
            ("server", entry.stored.name.clone()),
            ("host", entry.address.authority()),
          ],
        )
        .to_string()
    },
  );
  page::icon_note("sharing-no-project-yet", Icon::ExternalLink, &words)
}

/// *Kontur moves into your account, so it shows only while you're signed in.* (`Ib9ca`): a first link moves `project`,
/// this computer's, into the account whose server it is, as a project and its server have the same owner.
pub fn moves_to_account(ctx: &Ctx, project: &str) -> Element {
  Row::new()
    .id("sharing-moves-to-account")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::InfoMuted)
    .child(icon(Icon::UserRound, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("sharing.share.movesToAccount", [("project", project)]))
        .id("sharing-moves-to-account-text")
        .variant(ExtraTypography::Prose.role())
        .flex(1.0),
    )
    .into()
}

/// *What linking sends* (`What linking sends` in `bWD9q`).
pub fn sends(ctx: &Ctx) -> Element {
  card(
    ctx,
    "sharing-sends",
    ("sharing.link.sends.title", "sharing.link.sends.sharedValue"),
  )
}

/// *What joining sends* (`What joining sends` in `AiCwO`): the ticked tasks only.
pub fn join_sends(ctx: &Ctx) -> Element {
  card(
    ctx,
    "sharing-join-sends",
    ("sharing.join.sends.title", "sharing.join.sends.sharedValue"),
  )
}

/// The card `id`: its title and what is shared with members, by their keys, what stays here and what never goes.
fn card(ctx: &Ctx, id: &str, (title, shared): (&str, &str)) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(ExtraSpacing::Inset.role())
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(Text::new(&ctx.t(title)).variant(OrchesterText::CardTitle.role()))
    .child(sends_row(
      &format!("{id}-shared"),
      &ctx.t("sharing.link.sends.shared"),
      &ctx.t(shared),
    ))
    .child(sends_row(
      &format!("{id}-stays"),
      &ctx.t("sharing.link.sends.stays"),
      &ctx.t("sharing.link.sends.staysValue"),
    ))
    .child(
      Row::new()
        .id(format!("{id}-never"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t("sharing.link.sends.never"))
            .variant(ExtraTypography::Note.role())
            .flex(1.0),
        ),
    )
    .into()
}

/// A line of the card: what, in a 180 column of `text_muted`, and the things, in `text_primary` (`Shared` `othgy`).
fn sends_row(id: &str, label: &str, value: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(
      Text::new(label)
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Px(SENDS_LABEL))
        .flex_shrink(0.0),
    )
    .child(
      Text::new(value)
        .id(format!("{id}-value"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0),
    )
}
