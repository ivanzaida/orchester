# lurq-ui — portable agent skill

Guidance for building UI with the `lurq` Rust UI toolkit. Written to be project-agnostic:
it teaches a survey step ("find this app's component library, theme, locales, design
source") and then the rules and toolkit contracts that apply regardless of which app you're
in.

```
lurq-ui/
├── SKILL.md                        # the skill itself — rules + workflow
├── README.md                       # this file: what it is, how to install
└── references/
    ├── lurq-patterns.md            # toolkit contracts and gotchas (read when wiring behavior)
    └── app-setup.md                # standing up a lurq app: features, theme, i18n, fonts, breakpoints
```

`SKILL.md` is the single source of truth. Everything below just points different agents at
it — don't fork the content.

## Claude Code

Drop the folder in and it loads itself; the YAML frontmatter carries the trigger
description.

- **One project:** `<repo>/.claude/skills/lurq-ui/`
- **Every project:** `~/.claude/skills/lurq-ui/`

## Codex

Codex reads `AGENTS.md` files, not `.claude/skills/`, so it needs a pointer. Add this to
the repo-root `AGENTS.md` (for one project) or `~/.codex/AGENTS.md` (for every project),
fixing the path to wherever the folder lives:

```markdown
Before touching the visual layer of a `lurq` app (adding or restyling a screen, panel, row,
field, dialog, or control; wiring click/drag/keyboard/scroll behavior; adding user-facing
text; fixing a layout/text/scroll defect), read `.claude/skills/lurq-ui/SKILL.md` and follow
it, along with `references/lurq-patterns.md` beside it. That file has YAML frontmatter for
Claude Code's skill loader — ignore it and read the body.
```

For an explicit `/lurq-ui` slash command, add a prompt file at
`~/.codex/prompts/lurq-ui.md` containing the same pointer.

## Any other agent

The content is plain Markdown with no tool-specific syntax beyond the frontmatter block.
Point the agent at `SKILL.md` however it takes instructions — a rules file, a system
prompt, or a `@`-mention in the request.

## Keeping it accurate

The rules are stable; the toolkit facts in `references/lurq-patterns.md` are point-in-time
observations of lurq's behavior. When one turns out to be stale (a bug got fixed upstream,
an API gained the missing capability), correct it there rather than working around it in an
app — and if the workspace patches lurq to a local checkout, fixing it upstream *is* the
in-scope move.
