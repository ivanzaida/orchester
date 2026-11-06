# ORC-663: how a chosen skill reaches Codex, checked on the live CLI (2026-11-05)

`codex --version`: `codex-cli 0.166.0`, Windows 11 Pro 10.0.26200, the person's own sign-in (ChatGPT account) and
`~/.codex/config.toml`, which was only read (its MCP servers were turned off for the probe's threads, as the chat
does). The model was the person's configured default, with effort `low`. The probe is `.tmp/orc663/probe.py`, which is
not committed. It starts `codex app-server --stdio` in the scratch folder `.tmp/orc663/project` with the Orchestrator
chat's arguments: `ISOLATION`, `SUB_AGENTS_OFF`, the folder marked untrusted, `windows.sandbox=elevated`,
`sandbox_workspace_write.writable_roots=[]`, `project_doc_max_bytes=0` and the chat's `CONFINEMENT`. Then it sends
`initialize`, `skills/list` (`forceReload`), optionally `skills/extraRoots/set`, and a turn on an ephemeral
`read-only` thread with `approvalPolicy: never`. The probe skill is `.tmp/orc663/chat-skills/msg-1/probe-skill/SKILL.md`
(`name: probe-skill`), whose body says to answer with exactly `CERULEAN-663`. The App Server protocol was read from
`codex app-server generate-json-schema --out .tmp/orc663/schema`. `TurnStartParams.input` takes a `skill` item
(`{type, name, path}`). `skills/extraRoots/set` takes `{extraRoots: [absolute path]}`, and `skills/list` takes
`{cwds, forceReload}`.

No turn was refused for the 5-hour limit.

## 1. What the chat's App Server loads today

`python -I .tmp/orc663/probe.py list`: `skills/list` with the chat's arguments lists Codex's bundled skills only,
`system` scope from `C:\Users\…\.codex\skills\.system\` (`imagegen`, `openai-docs`, `review-agent`, `skill-creator`,
`skill-installer`). The person has no skills of their own in `~/.codex/skills` or `~/.agents/skills`.

## 2. A skill named by path outside every root is not found

`python -I .tmp/orc663/probe.py item text`. The input was the text `$probe-skill go` plus
`{"type":"skill","name":"probe-skill","path":"…\\chat-skills\\msg-1\\probe-skill\\SKILL.md"}`, and in the second
turn the text alone:

- `[item] agentMessage: "`probe-skill` isn’t available in this session. Paste its `SKILL.md` instructions, and I’ll
  follow them."`
- `[text] agentMessage: "`probe-skill` isn’t available in this session. Please provide its `SKILL.md` contents so I can
  follow it."`

So Codex reads no skill from an arbitrary folder; the folder must be registered (as ORC-325 suspected).

## 3. A folder registered with `skills/extraRoots/set` makes `$<slug>` run the skill

`python -I .tmp/orc663/probe.py extraitem extratext`:

- `skills/extraRoots/set -> {}`.
- `skills/list` then adds `('user', 'probe-skill', True, '…\\chat-skills\\msg-1\\probe-skill\\SKILL.md')` to the
  bundled ones.
- `[extraitem] agentMessage: "CERULEAN-663"`: the text `$probe-skill go` with the skill item.
- `[extratext] agentMessage: "CERULEAN-663"`: the text `$probe-skill go` alone.

## 4. The registration is App Server's alone

`python -I .tmp/orc663/probe.py list` after run 3: a new App Server lists the bundled skills only, not `probe-skill`.
Nothing is written to the configuration. `skills/extraRoots/set` takes no file, unlike `skills/config/write`, which
Orchester never sends. The person's `config.toml` was not touched.

## 5. The person's own skills: the gap, and how they are kept out

`python -I .tmp/orc663/probe.py list scratch` points `CODEX_HOME`, `USERPROFILE` and `HOME` at
`.tmp/orc663/home`, whose `.codex/skills/my-own/SKILL.md` and `.agents/skills/agents-own/SKILL.md` stand in for the
person's own skills. This run starts no turn, since that home has no sign-in. With the chat's arguments,
`skills/list` showed:

- `('user', 'my-own', True, '…\\home\\.codex\\skills\\my-own\\SKILL.md')`, next to the bundled ones.

So the isolation overrides (`features.plugins=false` and the others) do **not** keep `$CODEX_HOME/skills` out. That is
true for steps too, which this task does not change. `~/.agents/skills` did not show, because Codex finds the home
folder by the system's known folder, not by `USERPROFILE`. The person's real `~/.agents/skills` does not exist.

`python -I .tmp/orc663/probe.py scratch disable-user extra` adds
`-c skills.config=[{path="…\\home\\.codex\\skills\\my-own\\SKILL.md",enabled=false}]` and registers the probe's
folder:

- `('user', 'my-own', False, …)`: disabled.
- `('user', 'probe-skill', True, …)`: the registered skill stays enabled.
- The bundled skills stay enabled.

## What was built on it (the person chose this on 2026-11-05)

- Before every Orchestrator turn on Codex, the App Server that already reads Codex's MCP server names also answers
  `skills/list` for the project folder (`app_server_config::chat_read_command`, `read_chat_configuration`). Every
  enabled skill except the bundled (`system`) ones is disabled by path in the turn's `skills.config`, together with
  the project folder's own skills. Only Codex's bundled skills and the chosen one load. When that read fails, the turn
  goes on with the server names unchanged and no skill of the person's disabled, as before, and the runner's log says
  so.
- For a message that carries a skill, the runner writes the chosen version (with its reference files) to
  `<project>/.orchester/chat-skills/<message>/<slug>/` and registers that folder with `skills/extraRoots/set` before
  `config/read` and the thread's start. The turn's input is `$<slug> <words>`, the attached files, then
  `{"type":"skill","name":<slug>,"path":<its SKILL.md>}` (`CodexChatSkill`). The folder is removed once App Server
  exits. A Codex that refuses the registration gets the message as kept, with the skill's text in its marked block. A
  message sent into a running answer (`turn/steer`) and every message of a run's Conversation keep the marked block,
  as on Claude Code.
- The composer chip's tooltip on the Orchestrator over Codex says Codex loads the skill as its own (`$<slug>`),
  without the person's own Codex skills.

## Not run

- A model turn with a skill of the person's own present: the person's real home has none, and the scratch home has no
  sign-in (credentials are never copied). The exclusion was shown on the same binary with `skills/list` (section 5).
- macOS: not checked; the requests and arguments are the same there.
