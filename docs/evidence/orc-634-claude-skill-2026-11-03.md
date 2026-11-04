# ORC-634: how a chosen skill reaches Claude Code, checked on the live CLI (2026-11-03)

Claude Code 2.1.316 (`claude --version`), Windows 11 Pro 10.0.26200, the person's own sign-in, model `haiku`
(`claude-haiku-5-5`), in the ORC-634 run's worktree, which holds the repository's own `.claude/skills`
(`rust-code-quality`, `lurq-ui`, `orchester-ui`, `design-verification`). The person's home folder holds skills of
their own and the `oh-my-claudecode` plugin with SessionStart hooks. Codex is not installed on this computer, so it
was not checked (scope decided 2026-11-03).

The probe plugin, in `.tmp/orc634/plugin/` (not committed): `.claude-plugin/plugin.json`
`{"name":"orchester","description":"…","version":"1.0.0"}` and `skills/probe-skill/SKILL.md`, whose front matter
names `probe-skill` and whose body says to answer with exactly `PERIWINKLE-634`.

## 1. The isolation Orchester keeps, with the plugin and without `--disable-slash-commands`

```
claude -p "/orchester:probe-skill" --output-format stream-json --verbose --model haiku --strict-mcp-config
  --setting-sources "" --plugin-dir .tmp/orc634/plugin --tools "" --no-session-persistence --max-turns 2
```

- The answer: `PERIWINKLE-634`, so the skill's body reached the model as a skill (`"result":"PERIWINKLE-634"`).
- `init` event, `skills`: `deep-research`, `orchester:probe-skill`, `design`, `design-sync`, `dataviz`,
  `update-config`, `verify`, `debug`, `code-review`, `simplify`, `batch`, `fewer-permission-prompts`, `doctor`,
  `loop`, `schedule`, `claude-api`, `workflow-authoring`, `run`, `run-skill-generator`, `plugin-authoring`. That is
  the chosen skill and the CLI's bundled skills only.
- `plugins`: `orchester` (`orchester@inline`, the probe folder) and the CLI's built-in `cc-plugin-*` ones.
- No hook started; no skill of the person's, of the repository's `.claude/skills` or of `oh-my-claudecode`.

## 2. The same without `--setting-sources ""`: what the isolation keeps out

```
claude -p "Say OK." --output-format stream-json --verbose --model haiku --strict-mcp-config
  --plugin-dir .tmp/orc634/plugin --tools "" --no-session-persistence --max-turns 1
```

- Five `SessionStart:startup` hooks ran, one adding a "project memory" context.
- `skills` added the person's own (`awesome-design-md`, `design-taste-frontend`, `pen-design`, `pptx`,
  `research-brief-export`, `research-method`), the repository's (`design-verification`, `lurq-ui`, `orchester-ui`,
  `rust-code-quality`) and about fifty `oh-my-claudecode:*` ones; `plugins` added
  `oh-my-claudecode@omc` from `~/.claude/plugins/cache`.

So `--setting-sources ""` is what keeps the person's skills, plugins and hooks out. Run 1 kept it, and none of them
loaded.

## 3. `--disable-slash-commands` also turns the plugin's skill off

```
claude -p "/orchester:probe-skill" --output-format stream-json --verbose --model haiku --strict-mcp-config
  --setting-sources "" --disable-slash-commands --plugin-dir .tmp/orc634/plugin --tools ""
  --no-session-persistence --max-turns 1
```

- `init`: `"skills":[]`, `"slash_commands":[]`; the plugin is listed but its skill is not.
- The answer: "The `/orchester:probe-skill` command isn't installed in this session, so it didn't run."

A turn that carries a skill therefore leaves `--disable-slash-commands` out, as the `/compact` turn does (ORC-309).

## 4. The words after the skill's command reach the model too

```
claude -p "/orchester:probe-skill and then also write the word TEAL on a second line" --output-format json
  --model haiku --strict-mcp-config --setting-sources "" --plugin-dir .tmp/orc634/plugin --tools ""
  --no-session-persistence --max-turns 1
```

- `"result":"PERIWINKLE-634\nTEAL"`: the skill ran, and the words after its command were read as its arguments.

## What was built on it

- A new turn of the Orchestrator chat on Claude Code whose message carries a skill: the runner writes the chosen
  version (with its reference files) as a plugin of that skill alone in `<project>/.orchester/chat-skills/<message>/`,
  starts the CLI with `--plugin-dir <it>`, without `--disable-slash-commands` and with `--setting-sources ""` kept,
  gives it `/orchester:<slug> <words>`, and removes the folder once the CLI exits
  (`orchester_core::connection::invocation::ClaudeChat`, `crates/runner/src/execution/chat/skill.rs`).
- Everything else gets the skill's text in a marked block before the words, as the message is kept
  (`orchester_core::conversation::skill`): a message sent now into a running Orchestrator turn (the CLI already
  started, so no plugin can be added), every message of a run's Conversation (a step's CLI is usually running
  already), native agents and the Orchestrator on Codex. The person chose this split on 2026-11-03.

## Not run

- The skill command through `--input-format stream-json` (how the chat gives the prompt): not probed, since it needs
  a pipe into the CLI. The `/compact` turn already takes a slash command that way (ORC-309), and the app's
  end-to-end test drives the same arguments through the fake CLI.
- Codex: not installed; its `$<slug>` is left to its own task, verified on a live Codex.
- macOS: not checked.
