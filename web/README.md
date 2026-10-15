# Orchester web apps

An npm workspace (`docs/architecture/server.md`, "The web workspace"): one `package-lock.json` for

| Package | What |
| --- | --- |
| `ui/` (`@orchester/ui`) | The UI kit both web apps share: components, design tokens, icons, fonts, i18n helpers and the checks' helpers. No words of its own: components take every string from the app |
| `cloud/` (`@orchester/cloud`) | Orchester Cloud's web front (`cloud/README.md`) |

The server admin (`server/`) joins later on the same kit.

```sh
npm ci --ignore-scripts      # from web/
npm run check                # lint, type check and unit tests of every package (also: npm --prefix web run check)
npm run test:pages           # the Cloud's page tests (next build + Playwright in the installed Edge)
npm run tokens               # regenerate ui/theme/variables.css from ui/theme/pencil-variables.json
```

**Design tokens.** `ui/theme/pencil-variables.json` is a snapshot of `data/orchester2.pen`'s variables (read through the
Pencil MCP, accent graphite). `ui/scripts/tokens.ts` turns it into `ui/theme/variables.css`; a test fails when the
two differ. Values the design uses that are not variables are mapped by hand, with their Pencil node ids, in
`ui/theme/scale.css` (shared) and an app's own `theme/` (`cloud/theme/account.css`). Those files are the only places
with colours, sizes or fonts; each package's `literals` test fails on one anywhere else, and ESLint refuses JSX text,
literal labels, inline styles and hex colours.

**Words.** Each app has its own `locales/<locale>.json`; `@orchester/ui/i18n` gives the typed translator and the parity
check the apps' tests run.
