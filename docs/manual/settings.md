# Settings reference

Settings live in several places:

| Where | What | Kept in |
|---|---|---|
| **Settings** (app menu, the switcher's menu, the ⚙ of the sidebar's usage card) › General · Notifications · Accounts · Automation · Orchester Cloud | Theme, plan-limit warning, this computer; notifications and the daily report; accounts; the MCP server and what automation may do; Orchester Cloud and team servers | Application database (secrets and tokens in the OS keychain) |
| **Project settings** (the switcher's menu, Ctrl+, / ⌘,) › General · Runs · Context · Connections · Secrets · Sharing | Project, GitHub, base branch, base-moved notices, repository agent files; approvals (Allow Autopilot, reviewer), estimates, limits, environment; sources, code index; connections; secrets; sharing | Project database (base branch and Allow Autopilot per computer; secret values in the OS keychain) |
| **Memory › Settings** | How task and project memory save; how much each agent gets | Project database |
| **Orchestrator settings** | The chat's connection, model and effort (versioned) | Project database |
| **Agent profile › Permissions** | Repository access, writable folders, commands, network, MCP servers, approvals | Project database |
| **Pipeline page** (Settings tab, step column) | Sources, what a run delivers, Deliver step, step gates, budgets, failure routes, step permissions | Project database |
| **New / Edit task** chips | Pipeline, delivers, limits, sources, module, priority, points, *Don't merge automatically* | Project database |

**Person-only settings.** Settings › **Automation**, **Accounts** and **Orchester Cloud**, Project settings › Runs ›
*Let Orchester estimate story points*, and the widening confirmations in Project settings (Allow Autopilot, environment,
adding a source, turning repository agent files on) take only a person's input. Input from an MCP client is refused
visibly: "A change brought by automation was not taken: only you change this setting, here in the window." Turning a
permission *off* is always taken. General and Notifications have no such gate.

**Saving.** General, Notifications and Automation save as they change (the header shows "All changes saved",
"Saving…" or "A change was not saved"). Project settings › General, Runs and Context save when a field is left or a
switch flipped; Connections and Secrets save per action. While an autosaving tab has an unsaved or failed change, the
other tabs wait ("Save or fix the changes in this section before opening another."). A change made elsewhere shows
"Changed outside this screen" with *Reload*.

## 1. App settings (`/settings`)

Tabs: **General · Notifications · Accounts · Automation · Orchester Cloud**. `/settings` opens on Automation.

### 1.1 General

| Group | Setting | Values | Notes |
|---|---|---|---|
| Appearance | **Theme** | Dark (default) · Light · System | *System* uses the OS appearance at app start; a later OS change is followed only after a restart. |
| Plan limits | **Warning threshold** | 1–100 %, default 80 | The share of a 5-hour or weekly window at which plan limits count as nearly used. App-wide; also set from Usage › Plan limits. |
| This computer | Version, Application data, App log | — | Each with *Copy*. |

### 1.2 Notifications

| Group | Setting | Default | What |
|---|---|---|---|
| Notify me when | **Needs you** | On | An agent waits for an approval, an answer or a message; a plan or a run waits for your decision. |
| | **Stopped** | On | A run stopped at a limit or was interrupted, a runner is offline, or a merge stopped. |
| | **Finished and info** | Off | A run is ready to merge, was merged or failed; a plan limit reached 90 % or reset; the Orchestrator proposes something. |
| How | **Sound** | On | The system notification sound. |
| | **Quiet hours** | Off (22:00 → 08:00 when on, 30-minute steps) | No notifications between these times; the icon badge still updates. |
| Daily report | **Daily report from the Orchestrator** | Off (*Deliver at* 18:00 when on) | Each open project's Orchestrator conversation gets a report of the day; a missed report comes at the next start (up to 7 days back). |

Only the installed app shows notifications (or a build started with `ORCHESTER_APP_NOTIFICATIONS=1`).

### 1.3 Accounts

The person's agent CLI accounts on this computer, shared by every project, in a group per CLI (*Claude Code*,
*Codex*). Each CLI always has **Default** (its own folder, which can be renamed but never removed).

- **Add account** (a page): *1 · Name and folder* — CLI, name, *A new folder Orchester creates* or *A folder you
  already use* → *Create folder and sign in* / *Add and check*; *2 · Sign in with the CLI* — Orchester opens a terminal
  running the CLI's own login and checks again when it closes.
- An **account page**: state, **Check again**, and **…** (*Rename*, *Sign in in a terminal*, *Remove*); groups
  **Sign-in** (signed in as, plan, folder, signed in with, checked with, last checked), **Plan limits** (5-hour and
  weekly windows; **Check now** for Claude Code runs the smallest possible turn on Haiku; Codex limits are read without
  a turn) and **Used by** (the connections on it). *Remove* moves its connections to another account and can move a
  folder Orchester created to the Recycle Bin / Trash.

All account actions are person-only. Orchester never reads, shows or copies a CLI's token.

### 1.4 Automation (the default tab)

#### Orchester MCP server

For the installed app:

- **Let agents connect to Orchester** — on by default. Off: nothing listens; the address and token stay.
- **Address** — `http://127.0.0.1:47821/mcp` by default; a **Port** field (1024–65535) with **Use port** moves the
  server in place. A taken port is reported, never avoided.
- **Access token** — kept in Windows Credential Manager / the macOS login keychain, never in a file. **Show** (hidden
  again after 30 s; meanwhile the MCP server refuses screenshots and window text), **Copy** (clipboard cleared after
  1 minute), **Rotate** ("Rotate the token?" → *Rotate token* / *Keep current token*; every registered agent must
  register again).
- **Connect an agent** — tabs *Claude Code* (available *In every folder (user)* or *In one folder only (local)*) and
  *Codex*, each with **Copy command**.
- **Agents connected** — clients since start, with name and version.

A QA copy or development build shows a *Server* row instead: a new port and token on each start, published in a
discovery file. See [mcp-server.md](mcp-server.md).

#### The window, approvals and the Orchestrator

Every row is off until a person turns it on (except as noted), and only a person can change it.

| Group | Setting | Default | What it permits |
|---|---|---|---|
| The window | **Look at the window** | Always on | Screenshots, the element tree, the app's log and state, for every client with the token. |
| | **Agents can control this window** | Off in release builds, on in development builds | Click and type, resize, change pages and the theme. Applies from the next start; `ORCHESTER_APP_MCP_WINDOW_CONTROL` overrides it for one run. |
| Approvals and permissions | **Let automation answer approvals** | Off | Clients may approve a waiting tool request once or deny it, answer questions, decide plans, continue stopped runs with retry/return/accept, choose the reviewer — recorded as *automation's*. Wider answers stay yours; allowing Autopilot in a project is always a person's. |
| | **Let automation change agent permissions** | Off | Clients may widen what agents may do: write access, network, commands, MCP servers, approving automatically, new CLI programs, environment variables, API providers, new tools and publications. Narrowing is always allowed. |
| The Orchestrator may | **Publish drafts** | Off | The chat publishes pipelines, profiles, instructions, skills and tools it drafts without your click. Widening drafts still need *Let automation change agent permissions*. |
| | **Answer agents** | Off | The chat answers agents' questions and messages a run's agents directly, and starts a turn by itself when an agent waits, a run ends, or the daily report arrives. Approvals and plan decisions stay yours. |
| | **Merge runs** | Off | The chat merges a run you asked it to merge (latest succeeded run, no local integration needed). |
| | **Merge finished runs on its own** | Off | Needs *Merge runs* and *Answer agents*. Merges a task's latest run whose every step passed, once, in finish order — never Blocked tasks, tasks with unfinished blockers, while starts are held, or tasks marked *Don't merge automatically*. Conflicts → it proposes sending the work back to an agent. |
| | **Continue interrupted runs** | Off | The chat continues a run interrupted by a crash, runner loss or *Quit now* from its last checkpoint (once; not again within 10 minutes). Limit stops, canceled runs and runs without worktree stay yours. |
| | **Write with git** | Off | The chat may commit, create/switch branches, pull, push (never force), merge, revert, tag and fetch when you ask. Reset, clean, branch/tag deletion, force push, discarding and rebase stay cards. |

### 1.5 Orchester Cloud

One column:

- The **account** card: name, *Two-factor on* / *Two-factor: off* with **Turn on…** (opens the cloud's security page),
  email and host, **Sign out** ("Sign out of Orchester Cloud on this computer?" → *Sign out and lock* / *Stay signed
  in*). A warning appears when few recovery codes are left.
- **This computer**: device, signed in since, *Other devices* (not in this app yet).
- **Invites**: waiting invites to projects and servers — **Accept** (connects this computer; needs two-factor and a
  sign-in with a code) or **Decline** (asked in place).
- **Team servers** with **Connect a server**. A server opens its own page: state and versions, *Server admin*, **…**
  (*Sign out…*), address and certificate, your role, *Projects on this computer* (*Open*, *Link a checkout…*), *Your
  other computers* (*Add a computer* — a pairing code), *Open server admin* / *Sign in to the admin*. A changed
  certificate must be checked (*It matches · trust it* / *It doesn't match · sign out*).
- **Connect a server** (a page): *Address* → *Compare the check code* (self-signed servers; *It matches* is
  person-only) → *Join* (with your Orchester account or an 8-character invite code) → *Your username there*.
- **Advanced** › *Cloud address* (*Change*): the *Orchester Cloud address* (default `https://orchester.cloud`) and an
  *API address* (default `https://api.orchester.cloud`; empty = `/api/v1` on the cloud address), *Use this address* /
  *Back to the default*. Changing it signs you out of the cloud; team servers keep working.

## 2. Project settings (`/project/<id>/settings`)

Tabs: **General · Runs · Context · Connections · Secrets · Sharing**.

### 2.1 General

| Group | Setting | Default | Notes |
|---|---|---|---|
| Project | Name, Key (prefix of task keys), Folder | — | Read-only in this version. |
| Repository | **GitHub repository** | detected | From the folder's `origin`; *Check again*. |
| | **GitHub account** | the account detected at setup | Pull requests are created only while `gh` is signed in as it; *Use <account>*. Orchester never switches `gh` accounts. |
| | **Base branch** | Checked-out branch | Or a named local/remote branch. Kept on this computer only. A branch that's gone refuses new runs. |
| | **When the base branch moves** | On | The next writing agent merges the moved base after its turn; moves within two minutes are one notice. |
| | **Repository agent files** | On | Whether agents may read the repository's own `AGENTS.md`, `CLAUDE.md` and skills folders (*What each CLI loads*: Codex may load `AGENTS.md`, `.agents/skills`, `.codex/skills`; Claude Code loads none either way). Turning it on is person-only. |

### 2.2 Runs

| Group | Setting | Default | Notes |
|---|---|---|---|
| Approvals | **Allow Autopilot for agents in this project** | Off | On: agents whose profile is set to Autopilot approve their own requests (work outside the worktree, pushes to a remote and other branches still ask you). Off: every agent here runs as Manual. Kept on this computer only. Saves as you flip it; turning it on is person-only (automation's change is put back), turning it off is always taken. |
| | **Reviewer** | First Claude Code connection with Haiku 4.5 | The connection, model and effort that judge Manual's requests before you. Saves as you change it; automation chooses another only with both *Let automation answer approvals* and *Let automation change agent permissions*. Not an API-provider connection. |
| | **Approved for every run** | — | The rules from *Approve for every run of this project* (QA app starts while `tools/qa-app.ps1` is unchanged), on a page of their own with *Revoke*. |
| Estimates | **Let Orchester estimate story points** | Off | Per project. Estimates new and changed tasks with the reviewer's model (tokens count toward the daily budget). Points you set are never changed. Person-only. |
| Limits (apply to new runs) | **Run budget** | No limit | Tokens prefilled in New task. |
| | **Daily budget** | 20,000,000 tokens | All runs, the chat, reviews and estimates; new work waits once it's used. Empty = no limit. |
| | **Time limit** | No limit | Minutes of active agent time per run, prefilled in New task. |
| | **Agents at once** | 4 (1–16) | Applies at once; lowering it stops no agent. Command steps use slots of their own. |
| | **QA apps at once** | 2 (1–16) | Applies to the next QA app a run starts. |
| Environment | **Variables** | None | *Edit* opens a `NAME=value` editor. Added to every agent CLI and command tool the project's runs start, on top of the cleaned environment (PATH, HOME, TEMP…); a command tool's own variables win. **Stored in plain text — never put secrets here** (use Secrets). Variables that can change how programs load need a person's confirmation; harmless ones (`RUST_LOG`, `RUST_BACKTRACE`, `CI`, `NO_COLOR`, `FORCE_COLOR`, `TZ`, `LANG`, `LC_*`) don't. |

Runs already started keep their budget and time limit.

### 2.3 Context

- **Sources** — folders and files outside the worktree that every run's agents may **read and search without
  asking** (*Add a folder or file* → *Add* → "Let agents read this without asking?"). Anything else outside the
  worktree asks first; sources are never writable; `.git` folders, Orchester's data, credential files (`.ssh`, `.aws`,
  `.env`…) and other projects' private state are always refused. Tasks and pipelines can add their own sources. Changes
  apply to runs that start later.
- **Code index** — **Use code index** (off by default). On: agents get a built-in skill and `find_symbol` that search an
  index of the run's starting revision. *Index* rows (Ready, Building, Failed, Outdated) with *Rebuild* and *Retry*.
  **Search tool** shows ripgrep's version and source; *Install ripgrep* runs winget (Windows) or Homebrew (macOS).

### 2.4 Connections

Agent providers (with **Add provider**), GitHub, and MCP servers (with **Add MCP server**; a switch per server turns it
on or off). See [user-guide.md › Connections](user-guide.md#connections-project-settings--connections).

### 2.5 Secrets

Environment segments *All · Development · Staging · Production*, **Add secret** (name, type, value, environment →
*Save secret*), **Import from project** (explains that secrets are not imported). A secret's page has **Replace
value**, **…** (*Copy name*, *Archive*), *Details* and *Used by*. Values live in the OS secret store and are never shown
again after saving.

### 2.6 Sharing

- A **local** project: *Local project* and **Share on a team server › Share…** → choose the server and a project there
  (or its address) → *Link to this project…* → *Link and upload N tasks* (optionally *Write orchester.toml into the
  repository*).
- A **linked** project: *Team server* (address, your role, members with *Manage*, which opens the server admin,
  repository), *What a shared project can't do yet* (Delivers and *Don't merge automatically* are not kept by the
  server), *Discovery file* (`orchester.toml`, *Write orchester.toml* — Orchester never commits it), and *Leave or make
  local* (**Leave…**, **Make local…**).

## 3. Memory settings (Memory › Settings)

| Setting | Default | Range |
|---|---|---|
| **Task memory** | Save automatically | or *Save after approval* |
| **Project memory** | Save after approval | or *Save automatically* |
| **Records per invocation** | 8 | 1–50 |
| **Tokens per invocation** | 3,000 | 500–20,000 |

Changes apply to the next agent invocation. Person-only.

## 4. Orchestrator settings (`/project/<id>/orchestrator/settings`)

*Profile* tab: **Project connection** (Claude Code or Codex; an API-provider connection works), **Model** (or *Other
model id…*), **Reasoning effort** (*CLI default*, Minimal … Max as the model lists). **Save draft**, **Publish vN** (a
project's first version publishes straight from the form); a reply already under way finishes on the version it
started with. The same choices are in the chat composer's runs-on picker. The *Actions*, *Context* and *Conversations*
tabs are not built yet.
