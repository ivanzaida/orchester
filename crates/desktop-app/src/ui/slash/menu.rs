//! The slash menu's panel (`Slash menu` `IQOLN` in `07` `jAiyu`, `v7Xwo` in `06` `yKWQx`): a `Menu` panel 640 wide,
//! its left edge at the composer's box and its bottom 8 over it. Under the head *Commands* the chat's commands; a line,
//! then under the head *Skills* the project's published skills (`graduation-cap`, the name in Geist Mono, the
//! description), which scroll after 8 rows; the keys under them ([`super::rows`]). A head says at its end how many of
//! its rows match once something is typed (*2 of 4*); else the commands' head the composer's own note and the skills'
//! how many are published. A section with no match is hidden; with nothing that matches at all (`07d` `G2ajn7`) the
//! menu says so, and that Enter sends the text as typed. While the agent the message goes to cannot take a skill,
//! every skill is faded and says why.

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Placement, Popup, ScrollVertical},
  core::ElementRef,
  node::{Element, dimension::Dimension},
};
use parking_lot::Mutex;

use super::{
  Pick, Run, Shown, SlashCommand, SlashField, argument, matched, query,
  rows::{Face, footer, heading, nothing, row},
  skills::{self, Read, SkillsOffer},
};
use crate::{
  theme::{ControlHeight, ExtraSpacing},
  ui::{
    icon::Icon,
    menu::{menu_panel, separator},
    message::message_text,
  },
};

/// The menu's width (`IQOLN`).
const WIDTH: f32 = 640.0;
/// How many skills show before their section scrolls (`06d` `lXQi1`, the rules).
const SKILL_ROWS: u16 = 8;

/// The slash menu of the composer named `id`, over its box `anchor`, offering `commands` and the project's `skills`;
/// `note` is what the commands' head says at its end while nothing is typed (the run's agent and step in a run's
/// Conversation); `send` sends the draft as typed when nothing matches, where the field's own Enter does not.
pub struct SlashMenuProps {
  pub id: &'static str,
  pub field: SlashField,
  pub anchor: ElementRef,
  pub commands: Vec<SlashCommand>,
  pub skills: SkillsOffer,
  pub note: Option<Arc<str>>,
  pub send: Option<Run>,
}

#[derive(Clone)]
struct Props {
  id: &'static str,
  field: SlashField,
  anchor: ElementRef,
  faces: Vec<Face>,
  skills: SkillsOffer,
  note: Option<Arc<str>>,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.field == other.field
      && self.faces == other.faces
      && self.skills == other.skills
      && self.note == other.note
  }
}

impl DevtoolsInspectable for Props {}

pub fn menu(ctx: &mut Ctx, menu: SlashMenuProps) -> Element {
  let faces = menu
    .commands
    .iter()
    .map(|command| Face {
      pick: Pick::Command(command.name),
      name: Arc::from(command.name),
      icon: command.icon,
      label: command.label.clone(),
      detail: command.detail.clone(),
      runs: command.does.is_some(),
      skill: false,
    })
    .collect();
  {
    let mut offered = menu.field.offered.lock();
    offered.commands = menu.commands;
    offered.send = menu.send;
  }
  let props = Props {
    id: menu.id,
    field: menu.field,
    anchor: menu.anchor,
    faces,
    skills: menu.skills,
    note: menu.note,
  };
  ctx.mount_keyed::<SlashMenu>(&format!("{}-slash", props.id), props)
}

/// The skills as the Skills section shows them: their rows and how many are published, or why they could not be read.
enum SkillRows {
  Shown(Vec<Face>),
  Unreadable(Arc<str>),
}

/// A row that matches what was typed, with its matched letters.
type Matched<'a> = (&'a Face, Range<usize>);

/// What the Skills section shows: how many skills are published, those that match, or why they could not be read.
struct SkillSection<'a> {
  total: usize,
  matched: &'a [Matched<'a>],
  unreadable: Option<Arc<str>>,
}

/// What the menu showed last: what was typed after the `/`, and what each row chooses and whether it can be.
struct Seen {
  typed: String,
  rows: Vec<(Pick, bool)>,
}

struct SlashMenu {
  /// What the menu showed last: its focus goes back to the first row that can be chosen when it changes.
  last: Mutex<Option<Seen>>,
}

impl Component for SlashMenu {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { last: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let field = &props.field;
    let text = field.draft.get();
    let typed = query(&text);
    if typed.is_none() && field.closed.get() {
      field.closed.set(false);
    }
    // While a badge's picker is open, the draft filters it and the picker shows instead.
    let picking = field
      .argument
      .with(|held| held.iter().find(|badge| badge.picking).map(|badge| badge.command));
    let focused = field.focused.get();
    let open = typed.is_some() && picking.is_none() && focused && !field.closed.get();
    let read = skills::read(ctx, &props.skills, open);
    let (Some(typed), true) = (typed, open) else {
      *field.shown.lock() = None;
      *self.last.lock() = None;
      return match picking {
        Some(command) if focused => argument::picker(ctx, (props.id, field, &props.anchor), command),
        _ => {
          field.forget_choices();
          Element::new()
        }
      };
    };
    let skills = skill_rows(ctx, field, &props.skills, read);
    let skill_faces = match &skills {
      SkillRows::Shown(faces) => faces.as_slice(),
      SkillRows::Unreadable(_) => &[],
    };
    let commands = matching(&props.faces, typed);
    let matched_skills = matching(skill_faces, typed);
    let rows: Vec<(Pick, bool)> = commands
      .iter()
      .chain(&matched_skills)
      .map(|(face, _)| (face.pick.clone(), face.runs))
      .collect();
    let focused = self.focused(field, typed, &rows);
    let any = !rows.is_empty();
    *field.shown.lock() = Some(Shown { rows, focused });
    let content = if !any && matches!(skills, SkillRows::Shown(_)) {
      vec![nothing(ctx, typed), footer(ctx, typed, false)]
    } else {
      let mut content = commands_section(ctx, &props, (typed, &commands), focused);
      let section = SkillSection {
        total: skill_faces.len(),
        matched: &matched_skills,
        unreadable: match &skills {
          SkillRows::Unreadable(why) => Some(why.clone()),
          SkillRows::Shown(_) => None,
        },
      };
      let skill_focus = focused.and_then(|at| at.checked_sub(commands.len()));
      content.extend(skills_section(ctx, &props, (typed, section), skill_focus));
      content.push(footer(ctx, typed, any));
      content
    };
    let panel = menu_panel(&format!("{}-slash-menu", props.id))
      .width(Dimension::Px(WIDTH))
      .with_children(content);
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    Popup::new(props.anchor.clone(), panel)
      .open_when(true)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false)
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(0.0, gap)
      .into()
  }
}

impl SlashMenu {
  /// The row with the focus among `rows` for `typed`: the first that can be chosen once either changed, as when the
  /// skills arrive after the menu opened. The focus is read tracked on every path, so the arrow keys draw the menu
  /// again.
  fn focused(&self, field: &SlashField, typed: &str, rows: &[(Pick, bool)]) -> Option<usize> {
    let focus = field.focus.get();
    let mut last = self.last.lock();
    if last
      .as_ref()
      .is_none_or(|seen| seen.typed != typed || seen.rows != rows)
    {
      *last = Some(Seen {
        typed: typed.to_owned(),
        rows: rows.to_vec(),
      });
      let first = rows.iter().position(|(_, runs)| *runs);
      if focus != first {
        field.focus.set(first);
      }
      return first;
    }
    focus.filter(|&at| rows.get(at).is_some_and(|(_, runs)| *runs))
  }
}

/// The rows of `faces` that match `typed` ([`face_matched`]), with the matched letters.
fn matching<'a>(faces: &'a [Face], typed: &str) -> Vec<Matched<'a>> {
  faces
    .iter()
    .filter_map(|face| face_matched(face, typed).map(|letters| (face, letters)))
    .collect()
}

/// Where `typed` is in the row of `face`: in its label, whose letters show in bold; or in a command's own name, which
/// the person types after the `/` (`/unwatch` for *Stop watching*), with no letters in bold.
fn face_matched(face: &Face, typed: &str) -> Option<Range<usize>> {
  matched(&face.label, typed).or_else(|| {
    (!face.skill)
      .then(|| matched(&face.name, typed))
      .flatten()
      .map(|_| 0..0)
  })
}

/// The skills `read` found as rows, faded with why while the agent cannot take one; the field offers them to choose.
fn skill_rows(ctx: &Ctx, field: &SlashField, offer: &SkillsOffer, read: Read) -> SkillRows {
  let skills = match read {
    Read::Ready(skills) => skills,
    Read::Reading => Vec::new(),
    Read::Failed(reason) => {
      field.offered.lock().skills.clear();
      let reason = message_text(ctx, &reason);
      return SkillRows::Unreadable(ctx.t_args("composer.slash.skillsUnreadable", [("reason", reason)]));
    }
  };
  let faces = skills
    .iter()
    .map(|skill| Face {
      pick: Pick::Skill(skill.id),
      name: Arc::from(format!("skill-{}", skill.slug)),
      icon: Icon::GraduationCap,
      label: skill.slug.clone(),
      detail: offer.refused.clone().unwrap_or_else(|| skill.description.clone()),
      runs: offer.refused.is_none(),
      skill: true,
    })
    .collect();
  field.offered.lock().skills = skills;
  SkillRows::Shown(faces)
}

/// *Commands* and the commands that match `typed`, the focus at `focused`; nothing when none matches.
fn commands_section(
  ctx: &Ctx,
  props: &Props,
  (typed, commands): (&str, &[Matched<'_>]),
  focused: Option<usize>,
) -> Vec<Element> {
  if commands.is_empty() {
    return Vec::new();
  }
  let filtered = !typed.is_empty();
  let end = if filtered {
    Some(count(ctx, commands.len(), props.faces.len()))
  } else {
    props.note.clone()
  };
  let mut content = vec![heading(ctx, ("commands", "composer.slash.commands"), end)];
  content.extend(commands.iter().enumerate().map(|(at, (face, letters))| {
    row(
      ctx,
      (props.id, &props.field),
      (face, letters.clone(), filtered),
      focused == Some(at),
    )
  }));
  content
}

/// A line after the commands, *Skills* and the skills that match `typed`, the focus at `focused` among them; or why
/// they could not be read. Nothing when no skill matches, or none is published.
fn skills_section(
  ctx: &Ctx,
  props: &Props,
  (typed, section): (&str, SkillSection<'_>),
  focused: Option<usize>,
) -> Vec<Element> {
  let filtered = !typed.is_empty();
  let skills = section.matched;
  let end = match section.unreadable {
    Some(why) => why,
    None if skills.is_empty() => return Vec::new(),
    None if filtered => count(ctx, skills.len(), section.total),
    None => ctx.t_args("composer.slash.published", [("count", section.total.to_string())]),
  };
  let shown_commands = props.faces.iter().any(|face| face_matched(face, typed).is_some());
  let mut content: Vec<Element> = shown_commands.then(|| separator(ctx)).into_iter().collect();
  content.push(heading(ctx, ("skills", "composer.slash.skills"), Some(end)));
  if skills.is_empty() {
    return content;
  }
  let rows = Column::new()
    .id(format!("{}-slash-skills", props.id))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .with_children(skills.iter().enumerate().map(|(at, (face, letters))| {
      row(
        ctx,
        (props.id, &props.field),
        (face, letters.clone(), filtered),
        focused == Some(at),
      )
    }));
  if skills.len() <= usize::from(SKILL_ROWS) {
    content.push(rows.into());
    return content;
  }
  let rows_high = f32::from(SKILL_ROWS) * ControlHeight::Md.value();
  let gaps = f32::from(SKILL_ROWS - 1) * ExtraSpacing::Tight.value();
  content.push(
    ScrollVertical::new(rows)
      .width(Dimension::Pct(100.0))
      .max_height(Dimension::Px(rows_high + gaps))
      .into(),
  );
  content
}

/// *2 of 4*: how many of a section's rows match.
fn count(ctx: &Ctx, shown: usize, total: usize) -> Arc<str> {
  ctx.t_args(
    "composer.slash.count",
    [("shown", shown.to_string()), ("total", total.to_string())],
  )
}
