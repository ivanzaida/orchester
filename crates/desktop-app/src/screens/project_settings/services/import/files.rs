//! What the checkout suggests, by the file it came from (`deploy/cloud/compose.yaml` `guQxn`): the file's path in
//! 12/600 `text_secondary` with how many of its rows are kept in 11.5 `text_muted` and its kind's 13 glyph at the
//! line's end (`Head` `vYHmk`), over its rows in one outline (`Rows` `ESR4L`). A proposal's row is its name in Geist
//! Mono with its kind's chip, what was found under it and, a line each, what could not be carried with why, and Keep or
//! Skip at its end (`postgres` `TRdnM`). What the file holds but cannot be proposed, and a proposal the project has
//! already, show Skip with why under their names and in the control's tooltip. Files that could not be read follow,
//! with why.

use std::{collections::BTreeSet, sync::Arc};

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::service::{RunData, ServiceName};
use orchester_workspace::agent_testing::import::{Draft, Proposal, Proposals, Skipped, Unmapped};

use super::super::{
  super::kit::{Group, SettingRow, Tone},
  words,
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsText},
  ui::{
    chip::TagChip,
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
    tooltip::Tooltip,
  },
};

/// A proposal as the person keeps or skips it: services and stacks have names of their own apart.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Pick {
  Service(String),
  Stack(String),
}

impl Pick {
  fn of(proposal: &Proposal) -> Self {
    let name = proposal.draft.name().to_owned();
    match proposal.draft {
      Draft::Service(_) => Self::Service(name),
      Draft::Stack(_) => Self::Stack(name),
    }
  }
}

/// The proposals the person skipped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Skips(pub(super) BTreeSet<Pick>);

impl DevtoolsInspectable for Skips {}

/// Whether `proposal` is kept: the project has no service or stack of its name yet, and the person did not skip it.
pub(super) fn kept(proposal: &Proposal, skipped: &BTreeSet<Pick>) -> bool {
  !proposal.exists && !skipped.contains(&Pick::of(proposal))
}

/// One file's rows: its proposals in the order they are proposed, then what it holds that is not proposed.
struct File<'a> {
  path: &'a str,
  proposals: Vec<&'a Proposal>,
  skipped: Vec<&'a Skipped>,
}

impl<'a> File<'a> {
  /// The file `path` among `files`, added after the others when it is not there yet.
  fn at<'f>(files: &'f mut Vec<Self>, path: &'a str) -> &'f mut Self {
    let index = match files.iter().position(|file| file.path == path) {
      Some(index) => index,
      None => {
        files.push(Self {
          path,
          proposals: Vec::new(),
          skipped: Vec::new(),
        });
        files.len() - 1
      }
    };
    &mut files[index]
  }
}

/// The files of `proposals` in the order they are first named.
fn files(proposals: &Proposals) -> Vec<File<'_>> {
  let mut files = Vec::new();
  for proposal in &proposals.proposals {
    File::at(&mut files, &proposal.source.file).proposals.push(proposal);
  }
  for skipped in &proposals.skipped {
    File::at(&mut files, &skipped.source.file).skipped.push(skipped);
  }
  files
}

/// A group per file of the checkout that suggests something, then the files that could not be read.
pub(super) fn file_groups(ctx: &mut Ctx, proposals: &Proposals, skipped: &Signal<Skips>) -> Vec<Element> {
  let chosen = skipped.get().0;
  let mut shown: Vec<Element> = files(proposals)
    .iter()
    .enumerate()
    .map(|(index, file)| file_group(ctx, (index, file), skipped, &chosen))
    .collect();
  if !proposals.unreadable.is_empty() {
    let rows = proposals
      .unreadable
      .iter()
      .enumerate()
      .map(|(index, unreadable)| {
        SettingRow::new(format!("settings-import-unreadable-{index}"), unreadable.file.clone())
          .mono_label()
          .toned(message_text(ctx, &unreadable.reason), Tone::Warning)
          .render(ctx)
      })
      .collect::<Vec<_>>();
    shown.push(
      Group::new(
        "settings-import-unreadable",
        ctx.t("project_settings.services.import.unreadable"),
      )
      .rows(rows)
      .into(),
    );
  }
  shown
}

fn file_group(
  ctx: &mut Ctx,
  (index, file): (usize, &File<'_>),
  skipped: &Signal<Skips>,
  chosen: &BTreeSet<Pick>,
) -> Element {
  let id = format!("settings-import-file-{index}");
  let kept = file.proposals.iter().filter(|proposal| kept(proposal, chosen)).count();
  let count = ctx.t_args(
    "project_settings.services.import.fileKept",
    [
      ("kept", kept.to_string()),
      ("count", (file.proposals.len() + file.skipped.len()).to_string()),
    ],
  );
  let mut rows: Vec<Element> = file
    .proposals
    .iter()
    .map(|proposal| proposal_row(ctx, proposal, skipped, chosen))
    .collect();
  rows.extend(
    file
      .skipped
      .iter()
      .enumerate()
      .map(|(at, passed)| passed_row(ctx, &format!("{id}-skipped-{at}"), passed)),
  );
  Group::new(&id, file.path.to_owned())
    .end(
      Text::new(&count)
        .id(format!("{id}-count"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
    .end(icon(file_glyph(file.path), IconSize::Base, PaletteColor::TextMuted))
    .rows(rows)
    .into()
}

/// The glyph of a file's kind: `braces` for `package.json`, `package` for `Cargo.toml`, and for a compose file the
/// design's `container`, which `lucide.ttf` maps nowhere, drawn as `database`.
fn file_glyph(path: &str) -> Icon {
  let name = path.rsplit('/').next().unwrap_or(path);
  match name {
    "package.json" => Icon::Braces,
    "Cargo.toml" => Icon::Package,
    _ => Icon::Database,
  }
}

fn proposal_row(ctx: &mut Ctx, proposal: &Proposal, skipped: &Signal<Skips>, chosen: &BTreeSet<Pick>) -> Element {
  let pick = Pick::of(proposal);
  let (kind, kind_key, exists_key) = match pick {
    Pick::Service(_) => (
      "service",
      "project_settings.services.import.kind.service",
      "project_settings.services.import.exists.service",
    ),
    Pick::Stack(_) => (
      "stack",
      "project_settings.services.import.kind.stack",
      "project_settings.services.import.exists.stack",
    ),
  };
  let name = proposal.draft.name();
  let id = format!("settings-import-{kind}-{name}");
  let (hint, control) = if proposal.exists {
    let why = ctx.t_args(exists_key, [("name", name)]);
    (why.to_string(), fixed_skip(ctx, &id, why))
  } else {
    (found(ctx, proposal), choice(ctx, &id, (pick, chosen), skipped))
  };
  SettingRow::new(id.clone(), name.to_owned())
    .mono_label()
    .beside(TagChip::new(format!("{id}-kind"), ctx.t(kind_key)))
    .hint(hint)
    .lines(unmapped_lines(ctx, &id, &proposal.unmapped))
    .control(control)
    .render(ctx)
}

/// What the checkout holds but Orchester cannot propose: its key in the file, why, and Skip.
fn passed_row(ctx: &mut Ctx, id: &str, passed: &Skipped) -> Element {
  let why = message_text(ctx, &passed.reason);
  let cannot = ctx.t("project_settings.services.import.cannotKeep");
  let control = fixed_skip(ctx, id, cannot);
  SettingRow::new(id.to_owned(), passed.source.key.clone())
    .mono_label()
    .hint(why)
    .control(control)
    .render(ctx)
}

/// Keep and Skip's words.
fn words_of(ctx: &Ctx) -> [Arc<str>; 2] {
  [
    ctx.t("project_settings.services.import.keep"),
    ctx.t("project_settings.services.import.skip"),
  ]
}

/// Keep or Skip for a proposal the person chooses.
fn choice(ctx: &Ctx, id: &str, (pick, chosen): (Pick, &BTreeSet<Pick>), skipped: &Signal<Skips>) -> Element {
  let keep = !chosen.contains(&pick);
  let [keep_words, skip_words] = words_of(ctx);
  let segments = [(true, "keep", keep_words), (false, "skip", skip_words)].map(|(keeps, part, label)| {
    let (skipped, pick) = (skipped.clone(), pick.clone());
    Segment::new(
      format!("{id}-{part}"),
      SegmentFace::Text(label),
      keep == keeps,
      move || {
        skipped.update(|Skips(skipped)| {
          if keeps {
            skipped.remove(&pick);
          } else {
            skipped.insert(pick.clone());
          }
        });
      },
    )
  });
  segmented(&format!("{id}-choice"), segments, Focus::Keyboard).into()
}

/// Skip, which stays chosen: the row cannot be kept, and the tooltip says why.
fn fixed_skip(ctx: &mut Ctx, id: &str, why: Arc<str>) -> Element {
  let [keep_words, skip_words] = words_of(ctx);
  let segments = [(false, "keep", keep_words), (true, "skip", skip_words)].map(|(chosen, part, label)| {
    // Neither segment changes anything: the row has no draft to keep, as the tooltip says.
    Segment::new(format!("{id}-{part}"), SegmentFace::Text(label), chosen, || {})
  });
  let control = segmented(&format!("{id}-choice"), segments, Focus::Pointer).describe("state", "fixed");
  Tooltip::new(format!("{id}-choice-why"), why).wrap(ctx, control)
}

/// What was found: a container's image or the command a service runs, or the services a stack starts, with what a
/// service needs.
fn found(ctx: &Ctx, proposal: &Proposal) -> String {
  let key = proposal.source.key.as_str();
  match &proposal.draft {
    Draft::Service(service) => {
      let program = service.program().as_str();
      let what = match service.run_data() {
        RunData::DatabaseContainer => ctx.t_args("project_settings.services.import.found.image", [("image", program)]),
        _ => {
          let command = std::iter::once(program)
            .chain(service.arguments().iter().map(|argument| argument.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
          ctx.t_args(
            "project_settings.services.import.found.command",
            [("key", key.to_owned()), ("command", command)],
          )
        }
      };
      let needs = (!service.needs().is_empty()).then(|| {
        let names = service.needs().iter().map(ServiceName::as_str).collect::<Vec<_>>();
        ctx.t_args("project_settings.services.needs", [("names", names.join(words::LIST))])
      });
      words::join([Some(what), needs])
    }
    Draft::Stack(stack) => {
      let names = stack.services().iter().map(ServiceName::as_str).collect::<Vec<_>>();
      ctx
        .t_args(
          "project_settings.services.import.found.stack",
          [("names", names.join(words::LIST))],
        )
        .to_string()
    }
  }
}

/// What of the source the draft does not carry, a line each: its key in Geist Mono and why.
fn unmapped_lines(ctx: &Ctx, id: &str, unmapped: &[Unmapped]) -> Vec<Element> {
  unmapped
    .iter()
    .enumerate()
    .map(|(index, unmapped)| {
      Row::new()
        .id(format!("{id}-unmapped-{index}"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Start)
        .child(
          Text::new(&unmapped.key)
            .id(format!("{id}-unmapped-{index}-key"))
            .variant(ProjectSettingsText::MonoHint.role())
            .nowrap(),
        )
        .child(
          Text::new(&message_text(ctx, &unmapped.reason))
            .id(format!("{id}-unmapped-{index}-reason"))
            .variant(ProjectSettingsText::Hint.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .into()
    })
    .collect()
}
