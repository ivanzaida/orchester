# Agent accounts: several Claude Code and Codex sign-ins

Proposed 2026-10-12 for the person's request: *users may have multiple Claude / Codex / etc. accounts; we need to be
able to set them up somehow*. Not approved yet; the open decisions are at the end. The design source is
`data/orchester2.pen`, root layer **Accounts** (`hgOl2`) and its components layer **Accounts · components**
(`Cgob0`). Read the encrypted document only through Pencil. Every name, email, path, plan, percentage and time in the
frames is illustrative fixture data (Ana, `ana@example.com`, Work, Personal, Freelance, Client, Kontur, Atlas,
Orbit), never a default or shipped data.

## What exists today (checked in the code on 2026-10-12)

- A project **connection** is a program on this computer (`crates/core/src/connection/mod.rs`, `Connection`): kind,
  executable, last check, models. It has no notion of an account.
- Orchester never signs in. It checks with `codex login status` and `claude auth status --json`
  (`adapter.rs`, `parse_codex_login_status`, `parse_claude_auth_status`) and reads only signed in / signed out /
  unknown; `loggedIn` is the only Claude field it reads.
- Agent CLIs start with a cleaned environment (`connection/invocation/mod.rs`, `ALLOWED`). `CODEX_HOME` and
  `CLAUDE_CONFIG_DIR` are on the allow-list and **passed through from Orchester's own environment**; nothing sets
  them per connection. So today every connection of a CLI uses whatever sign-in that CLI has in the environment
  Orchester was started from.
- Plan-limit readings are kept per provider: `PlanReadings` says *one account per CLI on this computer*
  (`connection/plan_limits/readings.rs`); the status bar item and Usage chart are per CLI.
- GitHub: `gh auth status --hostname <host> --json hosts` names the active account; the project can expect a login
  (`GitHubState::WrongAccount`, advice `gh auth switch`). `GH_CONFIG_DIR` and `GH_TOKEN` are left in place
  (`connection/github/cli.rs`).
- The permission rules already treat the home folder's dot folders and application data (`AppData`, `Library`) as
  credential locations an agent's commands may not read without asking (`permission/category/paths.rs`,
  `holds_credentials`).

## CLI mechanics

Checked 2026-10-12 on Windows 11 with Claude Code 2.1.296 and Codex CLI 0.159.2, in empty scratch folders under
`.tmp/design-accounts/probe/` (no sign-in, no model turn, no real account read):

| Fact | Status |
|---|---|
| `CLAUDE_CONFIG_DIR=<folder> claude auth status --json` answers `loggedIn: false`, `authMethod: "none"`, and `configDirectory: <folder>`; claude writes `.claude.json` into that folder. The variable isolates the sign-in and the user state. | verified |
| `claude auth login` exists; options `--claudeai` (subscription, default), `--console` (API billing), `--sso`, `--email <email>` (pre-fills the login page). `claude auth logout` exists. | verified |
| A signed-in `claude auth status --json` also names the email, organisation and subscription. | **unverified** (we did not read a real account on purpose); the design shows identity and plan *if the CLI reports them* |
| `CODEX_HOME=<folder> codex login status` answers `Not logged in`; `codex login` signs in (browser), `--device-auth` for a device code; `--with-api-key` / `--with-access-token` read a secret from stdin. Orchester never uses those two. | verified |
| `codex app-server` (v2 protocol, schema generated with `generate-json-schema`) has `account/read` → `{account: {type: "chatgpt", email, planType} \| {type: "apiKey"} \| …, requiresOpenaiAuth}`, `account/rateLimits/read`, `account/login/start` (`chatgpt` → `authUrl`; `chatgptDeviceCode` → `verificationUrl` + `userCode`), `account/login/completed`, `account/updated`, `account/logout`. | verified (schema); behaviour per home unverified |
| Codex keeps its sign-in in `auth.json` in `CODEX_HOME` unless `cli_auth_credentials_store` moves it to the OS keyring; whether a keyring entry is per home. | **unverified** |
| macOS: Claude Code keeps its OAuth sign-in in the login Keychain; whether each `CLAUDE_CONFIG_DIR` gets its own Keychain item. | **unverified, blocking for macOS** — must be checked on a Mac before Claude accounts other than Default ship there (state `E96CI` › *Not supported on macOS yet*) |
| Claude Code reports plan limits only during a turn (`rate_limit_event`); Codex can be asked any time (`account/rateLimits/read`). | Claude: rechecked 2026-10-20 on 2.1.296, no turn-free read; Codex read: schema and a signed-out scratch home checked 2026-10-20 (0.137.0), see [Plan limits read on their own](#plan-limits-read-on-their-own) |
| gh: `GH_CONFIG_DIR` isolates `hosts.yml`; gh itself holds several accounts per host with one active (`gh auth switch` changes it for every program). Git pushes go through git's credential helper, which follows `GH_CONFIG_DIR` only if git runs with it. | partly from gh docs; push path **unverified** |

Each folder is also the CLI's **user configuration**, so an account brings its own:

- Codex: `config.toml` (user MCP servers, which the runner already turns off per thread by name after `config/read` —
  now read in that account's folder), `rules/` (kept as the person's choice; the enforcement panel's *Your own Codex
  rules still apply* must then name the account), hooks (a configuration with hooks refuses the step, per account),
  `skills/`, `AGENTS.md`, `sessions/` (thread rollouts).
- Claude Code: `settings.json`, `CLAUDE.md`, `skills/`, `agents/`, plugins, `projects/` (session transcripts).

A new folder starts empty: none of `~/.claude` or `~/.codex` comes along (copying settings is open decision 6).

## Recommended model

**Accounts are app-wide and belong to the person; a project connection chooses one; agent profiles stay as they
are.**

- *Settings › Accounts* (new tab between *Automation* and *Servers*) lists the person's accounts by CLI. An account
  is: CLI (Claude Code, Codex), a name (*Work*), a folder (the config home) and its last check (identity, plan,
  status). Accounts live in the application database (`orchester.db`) like servers, not in a project.
- Each CLI has a **Default** account: the CLI's own folder, i.e. whatever `CLAUDE_CONFIG_DIR` / `CODEX_HOME` the
  environment has, else `~/.claude` / `~/.codex`. It is always there, can be renamed, can't be removed. Every
  existing connection migrates to Default, so nothing changes for anyone who never adds an account.
- An agent CLI connection gains an **Account** field (program + account = connection). Connection names suggest the
  account (*Claude · Work*). Two connections of one CLI on different accounts can sit side by side in a project.
- Agent profiles keep choosing a connection; their menu shows each connection's account. Pipeline steps keep
  choosing profiles; the step inspector reads back the account.
- Runs pin the connection (as today) and therefore the account, recorded at the start like versions.

Why not per project: a sign-in belongs to a person, not a repository; signing in again per project is busywork,
plan limits are per account and shared by every project, and one folder per sign-in avoids duplicate homes. Why not
on the profile: *connection = how and as whom the CLI runs* is already the copy (*The agent CLI and sign-in this
profile runs on*), the profile stays small, and the pinning rules need no change.

**How a run uses it.** For a non-Default account Orchester sets `CLAUDE_CONFIG_DIR` / `CODEX_HOME` to the account's
folder for every process of that connection: the agent, the connection test (version, sign-in, models), the
auto-mode reviewer on that connection, plan-limit reads. Default leaves the variable as the environment has it
(today's behaviour). Resuming a session or thread uses the same account, because the session lives in that folder.

**Folders.** Orchester creates new ones next to its database: Windows
`%LOCALAPPDATA%\Orchester\accounts\<cli>-<name>`, macOS `~/Library/Application Support/Orchester/accounts/<cli>-<name>`
(both from `orchester_store::location`). The folder name is fixed at creation; renaming the account doesn't move it.
A person can instead bring a folder they already use (one they pointed `CLAUDE_CONFIG_DIR` at); Orchester refuses a
folder inside any project, worktree or git repository and a folder another account uses.

**GitHub.** It fits the same list later (*GitHub · Default* = gh's own config; extra accounts = `GH_CONFIG_DIR`
folders), but not in v1: gh already holds several accounts per host and the project already checks the expected
login, while a per-account `GH_CONFIG_DIR` must also reach `git push` through the credential helper, which is
unverified. v1 keeps the GitHub connection as is; the accounts list says so in its footnote.

## Frames

All in **Accounts** (`hgOl2`, x 63228, y 2000), dark, 1440 wide screens and 1896 wide state boards, each with its
`Label · …` 32 above. Exports at scale 1: `.tmp/design-accounts/<node id>.png`.

| Node | Frame | What it shows |
|---|---|---|
| `t098v` | Settings · Accounts · Claude account signed in | List by CLI (Claude Code: Work signed in, Personal signed in in a brought folder, Default not signed in; Codex: Default signed in, Client sign-in expired), Work's details: identity, plan, sign-in method, folder with Copy / Open, checked-with program, plan windows, what runs get, the no-token sentence |
| `IOyJl` | Settings · Accounts · Add account · name and folder | In-place panel (no dialog): steps 1–2, CLI option cards, Name, Folder (new folder Orchester creates / a folder you already use), *Create folder and sign in*; the list shows *Freelance · Not added yet* |
| `ohqHW` | Settings · Accounts · Add account · sign in in a terminal | Step 2: Orchester opened a terminal running `claude auth login` for the folder; waiting line, *Check again*, the PowerShell command for one's own terminal, *Finish later* / *Open the terminal again* |
| `GE7a3` | Settings · Accounts · sign-in step · states | Signed in (Claude, Codex with `account/read`), still not signed in after the terminal closed, same account as another, macOS Terminal + zsh command, CLI not found, folder inside a repository, folder already an account's |
| `E96CI` | Settings · Accounts · account states | Row + details for Checking, Not signed in (with command), Sign-in expired, CLI not found, Sign-in unknown, Folder missing, Same account as another, Not supported on macOS yet |
| `HzJSR` | Settings · Accounts · Work · Used by · remove in place | *Used by 4* tab with connections per project; in-place *Remove Work?* with the replacement account select, the finished-runs line and the opt-in *Also move the folder to the Recycle Bin* |
| `yEJPZ` | Settings · Accounts · rename and remove · states | Rename in the header; remove unavailable while a run is on the account; remove an account in a brought folder (folder stays, *No account* option); Default can't be removed |
| `WaKJN` | Connections · Add connection · Claude Code CLI · choosing its account | New *Account* row after *Program*, menu open: accounts with identity, plan use and state, *Add or manage accounts…*; check reads *ana@example.com on Work* |
| `M42rM` | Connections and agents · which account · states | Connections list with the account in each row, inspector with an *Account* row, a connection whose account is signed out, the agent profile's connection menu with accounts, the pipeline step read-back |
| `EwAhH` | Task · Summary · stopped at an account's plan limit | *Stopped at plan limit*; *Continue this run*: Work's windows, *Wait for Work, then continue* (chosen) or *Continue now on another account* with the connection select; *Accounts in this run*; *This run › Account*; status bar in red for Work only |
| `XPjSc` | Plan limits per account · status bar, waiting runs and Usage · states | Status bar with two Claude accounts, one account at its limit, the hint per account, a new run waiting for its account with *Start now on Personal*, Usage legend per account |

Components (`Cgob0`, x 71988, y 0): `I6Ebu` *Account group head*, `LW6D5` *Account list row*, `B9myCE` *Plan window
row* (window, `Meter`, used share, reset). Everything else reuses existing components: `S5CZ57`'s layout,
`vhBEA`, `Q91kML`, `j6ZUhL`, `PEnj5`, `R3Pyx`, `RWODj`, `QR1ER`, `d5xipm`, `g8tsS`/`Imh8O`, `i5CrTt`/`kjk4x`,
`JFXZg`, `pV2IA`, `LoIZ8`, `GOSxB`, `paUqV`, `wnnsD`, `XO1mE`/`p5L7N`, `wtAQI`, the buttons. No existing screen or
component was changed. The Settings screens drawn before (`hUaXB`, `wratq`, …) don't show the *Accounts* tab yet;
add it to them once the tab is approved.

## States

| State | Row / header | Meaning for runs | Action |
|---|---|---|---|
| Signed in | circle-check `$success`, *Signed in*; identity and plan as reported | runs start | Check again |
| Checking… | loader | last known values stay | — |
| Not signed in | circle-alert `$warning` | runs on it don't start; the run says which account | Sign in in a terminal, command, Check again |
| Sign-in expired | circle-alert `$warning`; *Was <email>* | same | Sign in again in a terminal |
| Sign-in unknown | circle-question-mark, muted | runs start (as `ConnectionStatus::SignInUnknown` today); a refusal is shown by the run | Check again |
| CLI not found | circle-x `$danger`; last known identity | Settings can't check; a connection with its own program still does | Check again |
| Folder missing | folder-x `$danger` | runs don't start | Choose folder…, Remove |
| Same account as another | signed in, identity line *same account as Work* | runs start; limits are shared | Keep both / Remove (at creation) |
| In use by a running run | — | Remove unavailable until it ends | Rename still allowed |
| Not supported on this platform yet | ban, muted | only Default is used for that CLI on this Mac | — |
| Connection whose account is signed out | connection status *Sign-in required* / *Client signed out* | its runs don't start | Open the account in Settings, Edit connection |

## Flows

1. **Add.** *Add account* opens the in-place panel. Step 1 (CLI, name, folder) creates the folder and saves the
   account as *Not signed in*. Step 2: Orchester opens a visible terminal running the CLI's own login with the
   folder's variable set (Windows: a new console window running PowerShell; macOS: Terminal), shows the same command
   to copy, and checks again when that terminal's command ends and on *Check again*. *Finish later* leaves it signed
   out. Orchester never sees what is typed, never passes a key or token, never reads the CLI's credential file.
2. **Bring a folder.** Choose a folder where the CLI is already signed in; Orchester checks it at once.
3. **Use.** In a connection, pick the account; profiles pick connections.
4. **Rename.** In place in the header; everything holding the account shows the new name.
5. **Remove.** In place: choose where its connections go (another account of that CLI, or *No account*, which stops
   their runs from starting); unavailable while a run is on it; Default can't be removed; a folder Orchester created
   can be moved to the Recycle Bin / Trash on request (never permanently deleted, never one it didn't create).

## Plan limits and the limit-reached behaviour

- Readings are keyed by account, not CLI (`PlanReadings` by account id). The status bar names an account only when
  the project uses more than one of a CLI; the hint lists every account the project's connections use; Usage draws
  one series per account and window, hue by CLI, the account in the label; the CLI filter becomes CLI and account.
- **v1 (proposed):**
  - A running agent never moves to another account by itself. Its session lives in its account's folder; moving it
    would mean copying session files between homes (undocumented) and would hand one organisation's work to another
    account's provider (a work account's code into a personal one) without the person saying so.
  - When an account hits its limit mid-step, the run stops at the CLI's refusal with a checkpoint, as *Stopped at
    plan limit*, and asks in place: **Wait for <account>, then continue** (default; at the reset Orchester resumes the
    same session on the same account; if the CLI refuses again, the run stops again and says so) or **Continue now
    on another account** (a new session of that step's profile on another connection of the same CLI and model, from
    the checkpoint, for this run only; the old conversation stays with the old account).
  - A new run whose step's account is at its limit waits (*Waiting for Work's plan limit · starts at 16:40*), holds no
    agent slot, starts by itself at the reset, and offers *Start now on <other connection>* for this run.
- **Later:** an ordered fallback list per connection (*if Work is at its limit, new runs use Personal*), applied to
  new runs only and shown on every run that used it; a warning before starting a run on an account above the
  warning threshold. (Reading Codex limits with `account/rateLimits/read` is done, below.)

## Plan limits read on their own

Built for ORC-55 (2026-10-20): the status bar, the hint, an account's details, `list_accounts` and the Usage page
showed an account's quota only as of the last agent turn that reported it, so an account without a run, or after a
reset, read stale or *Not reported yet*.

- **Codex: read without a turn.** Each Codex account someone may be signed in to (not checked yet, signed in or
  unknown; its folder there) is read through App Server's `account/rateLimits/read` with the account's CLI (the
  program its last check ran, else `codex` on the `PATH`) and `CODEX_HOME` set to its folder: `codex app-server
  --stdio` with the agents' isolation overrides, `initialize`, `initialized`, the read; no thread, no turn, no token.
  The answer is parsed as `account/rateLimits/updated` is: the `codex` bucket (`rateLimitsByLimitId.codex`, else
  `rateLimits`), windows by `windowDurationMins`, credits, `rateLimitReachedType`. Code:
  `orchester_core::connection::plan_limits::read`, the reader `orchester_workspace::plan_limits::reads`.
- **When.** At app start; when an account signs in or changes; when the status bar's plan limits hint or the Usage
  page opens; once after a reset the last reading named passes; every 5 minutes only while the hint or a Usage page
  is shown. Nothing polls otherwise. A failure waits 1 minute, doubled per failure in a row up to an hour; a provider
  holding requests back (429) 5 minutes, doubled; an account nobody is signed in to waits until it changes
  (`plan_limits::schedule`). Only the app's own window starts the reader (`Launch::read_plan_limits` in `main`): a
  test or a QA copy never reads a person's real account.
- **Claude Code: no supported turn-free read.** Claude Code 2.1.296 has no command or flag that reports plan usage
  without a turn (`claude --help`, `claude auth --help`; `auth status --json` reports the sign-in only, `/usage` is a
  slash command of an interactive session without machine-readable output). Orchester never reads, copies or sends
  the CLI's credentials and calls no undocumented endpoint with them. The person was offered (a) turn-only readings
  for Claude, or (b) an explicit *Check now*; **decided 2026-10-20: (b)**.
- ***Check now*** (Settings › Accounts › a Claude Code account › *Plan limits*). The smallest print-mode turn Claude
  Code runs, isolated as auto mode's reviewer is: `claude -p --output-format stream-json --verbose --model haiku
  --strict-mcp-config --setting-sources "" --disable-slash-commands --no-session-persistence --tools ""
  --system-prompt "Reply with one word."`, the prompt *Say OK.* on standard input, in an empty folder that goes
  afterwards, with the account's `CLAUDE_CONFIG_DIR`. Its `rate_limit_event`s are the reading. **Cost:** one turn on
  Haiku on the account's plan, a few dozen input tokens and a couple of output tokens by its shape (not measured on a
  real account); the account's note says it spends tokens, and it never runs by itself. It says why it gave no
  reading: nobody signed in, no plan limits (an API key), the CLI missing or refusing, a timeout, or a copy that does
  not read on its own.
- **Kept and shown.** A read is a reading like a turn's (`PlanReadings::observe_from`): per account, with its time and
  source (`ReadingSource`: turn or read); the newest from either source wins, and nothing is estimated between
  readings. Reads are kept 90 days in `orchester.db` (`plan_limit_reads`), which the Usage page's history and an
  account's details read beside the projects' turn reports. The hint says each account's age and source (*as of 2 min
  ago · read*, *as of 3 h ago · from a turn*); the account's details say *read …* or *reported … · from a turn*; the
  Usage table and tooltip say *Read by Orchester*. `list_accounts` returns each account's latest reading with its
  source, `observed_at`, `age_seconds`, state and windows, a window whose reset has passed since with its use unknown.
- **Not in the design yet:** the hint's age line, *Check now* and its error, and the new notes are not drawn in
  `orchester2.pen`; they reuse the hint's note style and the account panel's secondary button.
- **macOS:** the same on both platforms; Claude Code accounts other than Default remain *Not supported on macOS yet*,
  so *Check now* shows only for Default there.

## Security and privacy

- No password, key or token is ever shown, stored, copied or passed by Orchester; it runs only status commands
  (`claude auth status --json`, `codex login status` or `account/read`) and the CLI's own interactive login in a
  terminal the person sees. Codex's `--with-api-key` / `--with-access-token` and App Server's `apiKey` /
  `chatgptAuthTokens` logins are never used.
- Account folders are outside every project, worktree and repository; Orchester creates them in its own application
  data and refuses a brought folder inside a repository, so a sign-in can't be committed.
- What an agent can see: the CLI process reads its own folder (it has to); the variable naming the folder is in the
  agent's environment, so its commands know the path. That folder is in `AppData` / `Library`, which the permission
  rules already treat as a credential location: an agent's command reading it asks the person. Brought folders under
  the home folder's dot folders are covered the same way; a brought folder elsewhere is refused unless it is under
  one of those (open decision 4).
- Email and plan are personal data shown only in the window; they are not put into prompts, variables, logs or run
  output. Domain MCP tools list accounts by name, CLI and state without the email (open decision 7).
- Choosing an account for a connection is a person's choice made in the window; over MCP it is read-only (the MCP
  person gate: moving work to another account can send code to another organisation's provider).

## Copy

Proposed namespaces: a new `accounts.json` (`accounts.*`, the Settings tab) and additions to `settings.json`
(`settings.tabs.accounts`), `connections.json` (`connections.account.*`), `summary.json` (`summary.planLimit.*`)
and `shell.json` (`shell.status.planLimits.*`). Main strings as drawn:

| Key | English |
|---|---|
| `settings.tabs.accounts` | Accounts {count} |
| `accounts.add` | Add account |
| `accounts.note` | Accounts belong to you on this computer; every project can use them. Each is a folder where the CLI keeps its own sign-in. GitHub keeps using gh's signed-in account for now. |
| `accounts.group.count` | {count} accounts |
| `accounts.default.name` | Default |
| `accounts.default.identity` | The CLI's own folder · sign in to use it |
| `accounts.status.{signedIn,checking,notSignedIn,expired,unknown,cliMissing,folderMissing,unsupported}` | Signed in · Checking… · Not signed in · Sign-in expired · Sign-in unknown · CLI not found · Folder missing · Not supported on macOS yet |
| `accounts.details.{signedInAs,plan,signedInWith,folder,checkedWith,lastChecked}` | Signed in as · Plan · Signed in with · Folder · Checked with · Last checked |
| `accounts.details.planNote` | Claude Code reports limits only while it runs, so they are as of the last run on this account. Use outside Orchester shows up with the next report. |
| `accounts.details.runsGet` | Orchester starts {program} with {variable} set to this folder. The CLI then uses this folder's sign-in, settings, skills and saved sessions; what is in {defaultFolder} does not apply. |
| `accounts.details.noToken` | {program} wrote the sign-in into this folder when you signed in. Orchester only runs {statusCommand} to read who is signed in; it never reads, shows or copies a token. |
| `accounts.add.title` / `.sub` | Add account / An account is a folder where a CLI keeps its own sign-in. Name it and choose its folder; next you sign in with the CLI's own login. |
| `accounts.add.folder.{new,existing}` | A new folder Orchester creates / A folder you already use |
| `accounts.add.create` | Create folder and sign in |
| `accounts.signIn.title` | Sign in with {program}'s own login |
| `accounts.signIn.waiting` | Waiting for the terminal to close · opened {time} |
| `accounts.signIn.ownTerminal` | Or run it in a terminal of your own ({shell}) |
| `accounts.signIn.{finishLater,openAgain,checkAgain}` | Finish later · Open the terminal again · Check again |
| `accounts.error.{insideRepository,folderTaken}` | This folder is inside the repository {repository}. Choose one outside every project and repository. / {account} already uses this folder. Two accounts can't share one. |
| `accounts.same.title` | Signed in as {email}, the same account as {account} |
| `accounts.remove.title` | Remove {account} from Orchester? |
| `accounts.remove.moveTo` | Move the connections to |
| `accounts.remove.noAccount` | No account · it stops starting runs |
| `accounts.remove.trash` | Also move {folder} to the Recycle Bin (macOS: to the Trash) |
| `accounts.remove.runningReason` | Remove after {run} ends, or stop that run first |
| `accounts.remove.defaultReason` | Default is {program}'s own folder, {folder}, which every {program} you start yourself uses. Orchester can't remove it; to stop runs using it, choose other accounts in your connections. |
| `connections.account.label` / `.description` | Account / Whose sign-in this connection runs on. Accounts are yours and shared by every project; add one or sign in to it in Settings › Accounts. |
| `connections.account.manage` | Add or manage accounts… |
| `summary.planLimit.stopped` | Stopped at {account}'s plan limit · checkpoint {commit} |
| `summary.planLimit.wait` / `.other` | Wait for {account}, then continue / Continue now on another account |
| `summary.planLimit.continueAt` | Continue at {time} |
| `summary.planLimit.waiting` | Waiting for {account}'s plan limit · starts at {time} |
| `shell.status.planLimits.reached` | {cli} · {account} limit reached · resets {time} |

## v1 and later

v1: Settings › Accounts for Claude Code and Codex (list, add with a new or brought folder, sign in in a terminal,
check, rename, remove in place); Default per CLI and the migration; the connection's Account field and every
read-back drawn here; plan readings per account; the limit-reached choice and waiting runs; macOS support for Codex
accounts, and for Claude accounts only once the Keychain check passes.

Later: GitHub accounts; fallback order per connection; copying settings from Default; Codex sign-in through App
Server (`account/login/start`, browser opened by Orchester) instead of a terminal (reading Codex limits outside runs
is built: [Plan limits read on their own](#plan-limits-read-on-their-own));
accounts on the future server (accounts stay per computer; a shared project's connection would name an account per
person).

## Decisions

Decided 2026-10-12 (user): 1 (app-wide, chosen by the connection), 2 (visible terminal running the CLI's own login,
plus the command to copy), 3 (stop at the limit; wait for the reset by default or continue on another account
explicitly; fallback order later) and 5 (careful removal) as recommended below. 6, 7 and 8 were not accepted as
recommended; still open. 4, 9 and 10 not answered yet.

## Open decisions for the person

1. **Where accounts live.** Recommended: app-wide in Settings › Accounts, chosen by the connection. Alternative:
   per project, or chosen on the agent profile.
2. **How sign-in starts.** Recommended: Orchester opens a visible terminal running the CLI's own login with the
   folder's variable set, plus the command to copy. Alternatives: only show the command; or, for Codex, App Server's
   `account/login/start` with the browser opened by Orchester (no terminal, still no credentials through Orchester).
3. **Limit reached.** Recommended v1: never move a running agent; stop at the limit and ask in place — wait for the
   reset (default, resumes the same session automatically) or continue now on another account as a new session for
   this run; new runs wait and offer *Start now on …*. Fallback order per connection later.
4. **Brought folders.** Recommended: allow a folder the person already uses, refuse one inside a repository or
   project, and allow it only under the home folder's dot folders or application data, where agents' reads already
   ask. Alternative: only folders Orchester creates.
5. **Removing.** Recommended: ask where the connections go (or *No account*), block while a run is on it, never
   delete a brought folder, move a created folder to the Recycle Bin / Trash only when the person ticks it.
6. **Settings in a new account.** Recommended v1: start empty and say so. Later: *Copy settings from Default*
   (`settings.json`, `config.toml`, skills; never credential or session files).
7. **Email in MCP and run records.** Recommended: runs and MCP name the account; the email stays in the window only.
8. **GitHub.** Recommended: not in v1 (keep the expected-login check); add as `GH_CONFIG_DIR` accounts once
   `git push` through the credential helper is verified per folder.
9. **Same account twice.** Recommended: allow, with the plain note that limits are shared. Alternative: refuse.
10. **macOS Claude accounts.** Recommended: verify per-folder Keychain items on a Mac before v1; until then show
    *Not supported on macOS yet* for Claude accounts other than Default.
