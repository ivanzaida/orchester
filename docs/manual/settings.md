# Settings reference

Settings live in several places:

| Where | What | Kept in |
|---|---|---|
| **Settings** (app menu, sidebar bottom) › General · Automation · Accounts · Orchester | Theme, plan-limit warning, this computer; the MCP server; *What automation may do*; daily report; notifications; accounts; Orchester Cloud and team servers | Application database (secrets and tokens in the OS keychain) |
| **Project settings** › General · Limits · Environment · Sources · Code index · Sharing | GitHub, repository agent files, base-moved notices, auto mode, base branch; budgets and slots; environment variables; read-only sources; code index; sharing | Project database (base branch per computer) |
| **Memory › Settings** | How task and project memory save; how much each agent gets | Project database |
| **Orchestrator settings** | The chat's connection, model and effort (versioned) | Project database |
| **Agent profile › Permissions** | Repository access, writable folders, commands, network, MCP servers, approvals | Project database |
| **Pipeline settings and step inspector** | Sources, what a run delivers, Deliver step, gates, budgets, failure routes | Project database |
| **Task form** | Run budget, time limit, sources, module, story points, *Don't merge automatically* | Project database |

**Person-only settings.** All of Settings › Automation, Accounts and Orchester, and the widening confirmations in
Project settings (auto mode, environment, sources, turning agent files on), take only a person's input. Input from an
MCP client is refused visibly: "A change brought by automation was not taken: only you change this setting, here in
the window." Turning a permission *off* is always taken.

**Saving.** App settings save as they change ("All changes saved"). Project settings › Limits saves when a field is
left; other tabs save on change. While a tab has an unsaved or failed change, the other tabs wait. A change made
elsewhere shows "Changed outside this screen" with *Reload*.

## 1. App settings

### 1.1 General

| Setting | Values | Notes |
|---|---|---|
| **Theme** | Dark (default) · Light · System | *System* uses the OS appearance at app start; a later OS change is followed only after a restart. |
| **Warning threshold** (Plan limits) | 1–100 %, default 80 | The share of a 5-hour or weekly window at which plan limits count as nearly used. App-wide; also set from Usage › Plan limits. |
| **This computer** | — | Version, the application data path and the app log path, each with *Copy*. |

### 1.2 Automation (the default tab)

#### Orchester MCP server

For the installed app:

- **Let agents connect to Orchester** — on by default. Off: nothing listens; the address and token stay.
- **Address** — `http://127.0.0.1:47821/mcp` by default; **Port** (1024–65535) + **Use port** moves the server in
  place. A taken port is reported, never avoided.
- **Access token** — kept in Windows Credential Manager / the macOS login keychain, never in a file. **Show** (hidden
  again after 30 s; meanwhile the MCP server refuses screenshots and window text), **Copy** (clipboard cleared after
  1 minute), **Rotate token** (every registered agent must register again).
- **Connect an agent** — ready-to-copy commands for **Claude Code** (user or local scope) and **Codex** (environment
  variable or `config.toml`).
- **Agents connected** — clients since start, with name and version.

A QA copy or development build shows *Server* instead: a new port and token on each start, published in a discovery
file. See [mcp-server.md](mcp-server.md).

#### What automation may do

Each row is off until a person turns it on, and only a person can change it.

| Setting | Default | What it permits |
|---|---|---|
| **Look at the window** | Always on | Screenshots, the element tree, the app's log and state, for every client with the token. |
| **Agents can control this window** | Off in release builds, on in development builds | Click and type, resize, change pages and the theme. Applies from the next start; `ORCHESTER_APP_MCP_WINDOW_CONTROL` overrides it for one run. |
| **Let automation answer approvals** | Off | Clients may approve a waiting tool request once or deny it, answer questions, decide plans, continue stopped runs with retry/return/accept, set auto mode — recorded as *automation's*. Wider answers stay yours. |
| **Let automation change agent permissions** | Off | Clients may widen what agents may do: write access, network, commands, MCP servers, approving automatically, new CLI programs, environment variables, API providers, new tools and publications. Narrowing is always allowed. |
| **Let the Orchestrator publish drafts** | Off | The chat publishes pipelines, profiles, instructions, skills and tools it drafts without your click. Widening drafts still need the previous switch. |
| **Let the Orchestrator answer agents** | Off | The chat answers agents' questions and messages a run's agents directly, and starts a turn by itself when an agent waits, a run ends, or the daily report arrives. Approvals and plan decisions stay yours. |
| **Let the Orchestrator merge runs** | Off | The chat merges a run you asked it to merge (latest succeeded run, no local integration needed). |
| **Let the Orchestrator merge finished runs on its own** | Off | Needs *merge runs* and *answer agents*. Merges a task's latest run whose every step passed, once, in finish order — never Blocked tasks, tasks with unfinished blockers, while starts are held, or tasks marked *Don't merge automatically*. Conflicts → it proposes sending the work back to an agent. |
| **Let the Orchestrator continue interrupted runs** | Off | The chat continues a run interrupted by a crash, runner loss or *Quit now* from its last checkpoint (once; not again within 10 minutes). Limit stops, canceled runs and runs without worktree stay yours. |
| **Let the Orchestrator write with git** | Off | The chat may commit, create/switch branches, pull, push (never force), merge, revert, tag and fetch when you ask. Reset, clean, branch/tag deletion, force push, discarding and rebase stay cards. |
| **Let Orchester estimate story points** | Off | Estimates new and changed tasks with auto mode's reviewer model (tokens count toward the daily budget). Points you set are never changed. |
| **Daily report from the Orchestrator** | Off (18:00 when turned on, 30-minute steps) | Each open project's Orchestrator conversation gets a report of the day; a missed report comes at the next start (up to 7 days back). |

#### Notifications

| Setting | Default | What |
|---|---|---|
| **Needs you** | On | An agent waits for an approval, an answer or a message; a plan or a run waits for your decision. |
| **Stopped** | On | A run stopped at a limit or was interrupted, a runner is offline, or a merge stopped. |
| **Finished and info** | Off | A run is ready to merge, was merged or failed; a plan limit reached 90 % or reset; the Orchestrator proposes something. |
| **Sound** | On | The system notification sound. |
| **Quiet hours** | Off (22:00 → 08:00 when on) | No notifications between these times; the icon badge still updates. |

Only the installed app shows notifications (or a build started with `ORCHESTER_APP_NOTIFICATIONS=1`).

### 1.3 Accounts

The person's agent CLI accounts on this computer, shared by every project. Groups *Claude Code* and *Codex*; each CLI
always has **Default** (its own folder, which can be renamed but never removed).

- **Add account**: CLI, *Name*, *Folder* — *A new folder Orchester creates* or *A folder you already use* → *Create
  folder and sign in* / *Add and check*. Then *Sign in with the CLI*: Orchester opens a terminal running the CLI's own
  login and checks again when it closes.
- Account details: state (*Signed in*, *Not signed in*, *Sign-in expired*, *CLI not found*, *Folder missing*, *Not
  supported on macOS yet*…), *Signed in as*, *Plan*, *Signed in with*, folder, **Plan limits** (5-hour and weekly
  windows with resets). **Check now** (Claude Code only) runs the smallest possible turn on Haiku to read limits.
  Codex limits are read without a turn.
- **Check again**, **Rename**, **Remove** (moves its connections to another account; optionally moves a folder
  Orchester created to the Recycle Bin / Trash). *Used by* lists the connections on the account.

All account actions are person-only. Orchester never reads, shows or copies a CLI's token.

### 1.4 Orchester

- **Orchester Cloud** account: *Signed in as*, display name, this device, cloud address; **Sign out** (locks the app
  until you sign in again); *Two-factor: on/off* with *Turn on…* (opens the cloud's security page). *Advanced · cloud
  address* sets a test cloud. The *Devices* tab is not built yet.
- **Invites**: waiting invites to projects and servers — **Accept** (connects this computer; needs two-factor and a
  sign-in with a code) or **Decline**.
- **Team servers**: each server's state, certificate, your role, linked projects, *Link a checkout…*, *Your other
  computers* / *Add a computer* (pairing code), *Open server admin*, *Sign out…*. A changed certificate must be checked
  (*It matches · trust it*).
- **Connect a server**: *Address* → *Compare the check code* (self-signed servers; *It matches* is person-only) →
  *Join* (with your Orchester account or an 8-character invite code) → *Your username there*.

## 2. Project settings

### 2.1 General

| Section | Setting | Default | Notes |
|---|---|---|---|
| Project | Project name, Project key, Project folder | — | Read-only in this version. |
| GitHub | **GitHub repository** | detected | From the folder's `origin`; *Check again*. |
| | **Expected GitHub account** | the account detected at setup | Pull requests are created only while `gh` is signed in as it; *Use signed-in account*. Orchester never switches `gh` accounts. |
| Agents and the repository | **Repository agent files** | On | Whether agents may read the repository's own `AGENTS.md`, `CLAUDE.md` and skills folders. Codex may load `AGENTS.md`, `.agents/skills`, `.codex/skills`; Claude Code loads none either way. Turning it on is person-only. |
| When the base branch moves | **Tell working agents when the base branch moves** | On | The next writing agent merges the moved base after its turn; moves within two minutes are one notice. |
| Auto mode | **New runs use auto mode** | Off | A reviewer model answers what the rules leave to you. Changing it is person-only. |
| | **Reviewer** | First Claude Code connection with Haiku 4.5 | *Project connection*, *Model*, *Reasoning effort*. Not on an API-provider connection. |
| | **Approved for every run** | — | Rules from *Approve for every run of this project* (QA app starts while `tools/qa-app.ps1` is unchanged), with *Revoke*. |
| Base branch | **Base branch** | Checked-out branch | Or a named local/remote branch. Kept on this computer only. A branch that's gone refuses new runs. |

### 2.2 Limits

| Setting | Default | Notes |
|---|---|---|
| **Default run budget** | No limit | Tokens prefilled in New task. |
| **Daily project budget** | 20,000,000 tokens | All runs, the chat, reviews and estimates; new work waits once it's used. Empty = no limit. |
| **Default time limit** | No limit | Minutes of active agent time prefilled in New task. |
| **Agents working at once** | 4 (1–16) | Applies at once; lowering it stops no agent. Command steps use slots of their own. |
| **QA apps at once** | 2 (1–16) | Applies to the next QA app a run starts. |

Runs already started keep their budget and time limit.

### 2.3 Environment

*Variables · NAME=value per line*, added to every agent CLI and command tool the project's runs start, on top of the
cleaned environment (PATH, HOME, TEMP…). A command tool's own variables win. **Stored in plain text — never put
secrets here** (use Secrets). Changes apply to the next agent or command. Variables that can change how programs load
need a person's confirmation; harmless ones (`RUST_LOG`, `RUST_BACKTRACE`, `CI`, `NO_COLOR`, `FORCE_COLOR`, `TZ`,
`LANG`, `LC_*`) don't.

### 2.4 Sources

Folders and files outside the worktree that every run's agents may **read and search without asking** (*Add source* →
"Let agents read this without asking?"). Anything else outside the worktree asks first; sources are never writable;
`.git` folders, Orchester's data, credential files (`.ssh`, `.aws`, `.env`…) and other projects' private state are
always refused. Tasks and pipelines can add their own sources. Changes apply to runs that start later.

### 2.5 Code index

**Use code index** — Off by default. On: agents get a built-in skill and `find_symbol` that search an index of the
run's starting revision. *Indexes* lists them (Ready, Building, Failed, Outdated) with *Rebuild* and *Retry*.
**Search tool** shows ripgrep's version and source; *Install ripgrep* runs winget (Windows) or Homebrew (macOS).

### 2.6 Sharing

A local project: *Task list · Local only* and **Link to a server project** (server, project or project address,
repository check) with *What linking sends*. A linked project: *Shared on <server>*, project address, *Discovery file*
(`orchester.toml`, *Write orchester.toml* — Orchester never commits it), *Members* (read-only; owners get *Manage members
and invites*, which opens the server admin), **Make local…** and **Leave…**.

The *Memory* tab here is disabled; memory settings live on the Memory page's *Settings* tab.

## 3. Memory settings (Memory › Settings)

| Setting | Default | Range |
|---|---|---|
| **Task memory** | Save automatically | or *Save after approval* |
| **Project memory** | Save after approval | or *Save automatically* |
| **Records per invocation** | 8 | 1–50 |
| **Tokens per invocation** | 3,000 | 500–20,000 |

Changes apply to the next agent invocation. Person-only.

## 4. Orchestrator settings

*Profile* tab: **Project connection** (Claude Code or Codex; an API-provider connection works), **Model** (or *Other
model id…*), **Reasoning effort** (*CLI default*, Minimal … Max as the model lists). **Save draft**, **Publish vN**; a
reply already under way finishes on the version it started with. The *Actions*, *Context* and *Conversations* tabs are
not built yet.
