//! The search tool agents get (`ganqV` in `i220W`): whether ripgrep was found and where, and while
//! it was not (`h6yMk`) what agents do instead with the exact install command, *Install ripgrep*
//! and *Check again*; the install running, failed, or unavailable without its installer (`NvR7K`).

use std::{sync::Arc, time::SystemTime};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_exec::ripgrep::{
  Source,
  install::{InstallError, InstallPlan, InstallUnavailable, Installer},
};

use super::super::general::{readback, sized_setting_row};
use crate::{
  code_index::{self, Install, RipgrepCheck, SearchTool},
  display,
  messages::Message,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

type Handler = Arc<dyn Fn() + Send + Sync>;

/// The setting's value column (`UdfsQ`) and the notice's button height (`h6yMk`).
const VALUE_WIDTH: f32 = 300.0;
const BUTTON_HEIGHT: f32 = 32.0;

/// The ripgrep row, then the notice while agents have no ripgrep or an install runs or failed.
pub fn section(
  ctx: &Ctx,
  check: Option<&Result<RipgrepCheck, Message>>,
  install: Option<&Install>,
  (check_revision, tick): (&Signal<u64>, &Signal<u64>),
) -> Vec<Element> {
  let check_again: Handler = {
    let revision = check_revision.clone();
    Arc::new(move || revision.update(|revision| *revision += 1))
  };
  // The tab shows the install running once it started.
  let started: Handler = {
    let tick = tick.clone();
    Arc::new(move || tick.update(|tick| *tick += 1))
  };
  let running = matches!(install, Some(Install::Running { .. }));
  let mut elements = vec![ripgrep_row(ctx, check, install)];
  let Some(Ok(check)) = check else {
    return elements;
  };
  let missing = !matches!(check.tool, SearchTool::Found { .. });
  let notice = match install {
    Some(Install::Running {
      installer,
      line,
      last_output,
      ..
    }) => Some(installing(ctx, *installer, line, last_output.as_deref())),
    Some(Install::Failed { installer, line, error }) if missing => Some(failed(
      ctx,
      (*installer, line, error),
      &check.install,
      (&check_again, &started),
    )),
    _ if missing => Some(not_found(ctx, &check.install, (&check_again, &started))),
    _ => None,
  };
  elements.extend(notice.filter(|_| missing || running));
  elements
}

/// *ripgrep* (`ganqV`): found with its version and where it came from, not found, or looking.
fn ripgrep_row(ctx: &Ctx, check: Option<&Result<RipgrepCheck, Message>>, install: Option<&Install>) -> Element {
  let (glyph, state, detail): (Icon, Arc<str>, Arc<str>) = match (install, check) {
    (Some(Install::Running { installer, started, .. }), _) => (
      Icon::Loader,
      ctx.t("settings.codeIndex.installing"),
      installing_readback(ctx, *installer, *started),
    ),
    (_, None) => (Icon::Loader, ctx.t("settings.codeIndex.checking"), Arc::from("")),
    (_, Some(Err(error))) => (
      Icon::AlertCircle,
      ctx.t("settings.codeIndex.checkFailed"),
      message_text(ctx, error),
    ),
    (_, Some(Ok(check))) => match &check.tool {
      SearchTool::Found { version, source } => (
        Icon::Check,
        match version {
          Some(version) => ctx.t_args("settings.codeIndex.found", [("version", version.as_str())]),
          None => ctx.t("settings.codeIndex.foundWithoutVersion"),
        },
        ctx.t(source_key(*source)),
      ),
      SearchTool::NotFound => (
        Icon::SearchX,
        ctx.t("settings.codeIndex.notFound"),
        ctx.t("settings.codeIndex.notFoundDetail"),
      ),
      SearchTool::Unavailable { detail } => (
        Icon::AlertCircle,
        ctx.t("settings.codeIndex.notCopied"),
        Arc::from(detail.as_str()),
      ),
    },
  };
  let value = vec![
    Row::new()
      .id("settings-code-index-ripgrep-state")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(glyph, IconSize::Dismiss, PaletteColor::TextPrimary))
      .child(
        Text::new(&state)
          .variant(TypographyStyle::Body)
          .color(PaletteColor::TextPrimary),
      )
      .into(),
    readback(&detail),
  ];
  sized_setting_row(
    VALUE_WIDTH,
    &ctx.t("settings.codeIndex.ripgrep"),
    &ctx.t("settings.codeIndex.ripgrepDetail"),
    value,
  )
}

fn source_key(source: Source) -> &'static str {
  match source {
    Source::OnPath => "settings.codeIndex.source.onPath",
    Source::Winget => "settings.codeIndex.source.winget",
    Source::CodexCli => "settings.codeIndex.source.codexCli",
    Source::ClaudeCode => "settings.codeIndex.source.claudeCode",
  }
}

/// `winget started at 14:31 · 12 s`.
fn installing_readback(ctx: &Ctx, installer: Installer, started: SystemTime) -> Arc<str> {
  let elapsed = started.elapsed().unwrap_or_default();
  ctx.t_args(
    "settings.codeIndex.installingReadback",
    [
      ("installer", &*ctx.t(installer_key(installer))),
      ("time", display::clock_time(started).as_str()),
      ("elapsed", &*message_text(ctx, &display::duration(elapsed))),
    ],
  )
}

fn installer_key(installer: Installer) -> &'static str {
  match installer {
    Installer::Winget => "settings.codeIndex.installer.winget",
    Installer::Homebrew => "settings.codeIndex.installer.homebrew",
  }
}

/// Agents fall back to slower search (`h6yMk`): the command *Install ripgrep* runs, or why it is
/// unavailable, and *Check again*.
fn not_found(
  ctx: &Ctx,
  plan: &Result<InstallPlan, InstallUnavailable>,
  (check_again, started): (&Handler, &Handler),
) -> Element {
  let copy = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(heading(&ctx.t("settings.codeIndex.slowTitle"), false))
    .child(detail(&ctx.t("settings.codeIndex.slowDetail"), false));
  let copy = match plan {
    Ok(plan) => copy
      .child(spaced(
        Text::new(&ctx.t("settings.codeIndex.commandLabel")).variant(TypographyStyle::Caption),
      ))
      .child(command(&plan.command_line()))
      .child(hint(&ctx.t("settings.codeIndex.hint")))
      .child(spaced(actions(
        ctx,
        install_button(ctx, ("settings.codeIndex.install", Icon::Download), Some(plan), started),
        check_again,
      ))),
    Err(unavailable) => copy
      .child(hint(&unavailable_text(ctx, unavailable)))
      .child(spaced(actions(
        ctx,
        install_button(ctx, ("settings.codeIndex.install", Icon::Download), None, started),
        check_again,
      ))),
  };
  notice("settings-code-index-ripgrep-notice", Icon::SearchX, false, copy.into())
}

/// Installing ripgrep (`NvR7K`): the command that runs, its last output, both buttons disabled.
fn installing(ctx: &Ctx, installer: Installer, line: &str, last_output: Option<&str>) -> Element {
  let installer = ctx.t(installer_key(installer));
  let running = Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Loader, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("settings.codeIndex.running")).variant(TypographyStyle::Caption));
  let output = last_output.map(|output| hint(&ctx.t_args("settings.codeIndex.lastOutput", [("line", output)])));
  let disabled = |label, glyph, id| {
    ActionButton::new(ctx.t(label))
      .icon(glyph)
      .id(id)
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT))
      .on_click(|| {})
      .enabled(false)
      .render(ctx)
  };
  let copy = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(heading(
      &ctx.t_args("settings.codeIndex.installingTitle", [("installer", &*installer)]),
      false,
    ))
    .child(detail(&ctx.t("settings.codeIndex.installingDetail"), false))
    .child(spaced(command(line)))
    .child(running)
    .with_children(output)
    .child(spaced(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(disabled(
          "settings.codeIndex.installing",
          Icon::Loader,
          "settings-code-index-install",
        ))
        .child(disabled(
          "settings.codeIndex.checkAgain",
          Icon::RefreshCw,
          "settings-code-index-check",
        )),
    ));
  notice("settings-code-index-ripgrep-notice", Icon::Download, false, copy.into())
}

/// ripgrep was not installed (`NvR7K`): what the installer said, *Try again* and *Check again*.
fn failed(
  ctx: &Ctx,
  (installer, line, error): (Installer, &str, &InstallError),
  plan: &Result<InstallPlan, InstallUnavailable>,
  (check_again, started): (&Handler, &Handler),
) -> Element {
  let installer = ctx.t(installer_key(installer));
  let text = match error {
    InstallError::Failed { code, message } => ctx.t_args(
      "settings.codeIndex.installFailedDetail",
      [
        ("installer", &*installer),
        ("code", code.map_or_else(String::new, |code| code.to_string()).as_str()),
        ("message", message.as_str()),
      ],
    ),
    InstallError::Run(error) => ctx.t_args(
      "settings.codeIndex.installNotStarted",
      [("installer", &*installer), ("message", error.to_string().as_str())],
    ),
  };
  let copy = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Tight.role())
    .child(heading(&ctx.t("settings.codeIndex.installFailedTitle"), true))
    .child(detail(&text, true))
    .child(spaced(command(line)))
    .child(spaced(actions(
      ctx,
      install_button(
        ctx,
        ("settings.codeIndex.tryAgain", Icon::RotateCcw),
        plan.as_ref().ok(),
        started,
      ),
      check_again,
    )));
  notice(
    "settings-code-index-ripgrep-notice",
    Icon::AlertCircle,
    true,
    copy.into(),
  )
}

fn unavailable_text(ctx: &Ctx, unavailable: &InstallUnavailable) -> Arc<str> {
  match unavailable {
    InstallUnavailable::Unsupported => ctx.t("settings.codeIndex.installUnsupported"),
    InstallUnavailable::InstallerMissing { installer } => {
      let program = match installer {
        Installer::Winget => ctx.t("settings.codeIndex.installer.winget"),
        Installer::Homebrew => ctx.t("settings.codeIndex.installer.homebrewProgram"),
      };
      ctx.t_args("settings.codeIndex.installUnavailable", [("installer", &*program)])
    }
  }
}

/// *Install ripgrep* or *Try again*: starts `plan`; without one it is unavailable, with a lock.
fn install_button(ctx: &Ctx, (label, glyph): (&str, Icon), plan: Option<&InstallPlan>, started: &Handler) -> Element {
  let button = ActionButton::new(ctx.t(label))
    .icon(glyph)
    .id("settings-code-index-install")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT));
  match plan {
    Some(plan) => {
      let (plan, started) = (plan.clone(), started.clone());
      button
        .on_click(move || {
          code_index::start_install(&plan);
          started();
        })
        .render(ctx)
    }
    // Without `on_click` the button renders as unavailable.
    None => button.render(ctx),
  }
}

fn actions(ctx: &Ctx, install: Element, check_again: &Handler) -> Element {
  let check_again = check_again.clone();
  let check = ActionButton::new(ctx.t("settings.codeIndex.checkAgain"))
    .icon(Icon::RefreshCw)
    .id("settings-code-index-check")
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT))
    .on_click(move || check_again())
    .render(ctx);
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(install)
    .child(check)
    .into()
}

/// The notice (`h6yMk`): its icon in an 18px slot beside `copy`, on the raised fill.
fn notice(id: &'static str, glyph: Icon, emphasized: bool, copy: Element) -> Element {
  let color = if emphasized {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  Row::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Start)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(glyph, IconSize::Notice, color))
    .child(copy)
    .into()
}

/// The notice's heading: 12 in the text color, medium when the install failed.
fn heading(text: &str, emphasized: bool) -> Text {
  let typography = if emphasized {
    ExtraTypography::NoticeHeading.role()
  } else {
    TypographyStyle::FieldLabel
  };
  Text::new(text).variant(typography).color(PaletteColor::TextPrimary)
}

/// The notice's detail: muted, or in the text color when the install failed.
fn detail(text: &str, emphasized: bool) -> Text {
  let text = Text::new(text)
    .variant(ExtraTypography::Detail.role())
    .width(Dimension::Pct(100.0));
  if emphasized {
    text.color(PaletteColor::TextPrimary)
  } else {
    text
  }
}

fn hint(text: &str) -> Text {
  Text::new(text)
    .variant(ExtraTypography::Hint.role())
    .width(Dimension::Pct(100.0))
}

/// The exact command in mono in a field-like box.
fn command(line: &str) -> Element {
  Row::new()
    .id("settings-code-index-command")
    .width(Dimension::Pct(100.0))
    .padding_vertical(SpacingSize::Sm)
    .padding_horizontal(ExtraSpacing::Stack.role())
    .background(PaletteColor::SurfacePanel)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(
      Text::new(line)
        .variant(ExtraTypography::MonoField.role())
        .selectable(true)
        .flex(1.0),
    )
    .into()
}

/// `element` a little further from what comes before it (the 4px spaces of `h6yMk`).
fn spaced(element: impl Into<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Xs)
    .child(element)
    .into()
}
