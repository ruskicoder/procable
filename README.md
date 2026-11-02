# procable

procable is an offline, cleaned fork of [Impeccable](https://github.com/pbakaus/impeccable), maintained by
[ruskicoder](https://github.com/ruskicoder) for [prompt-orchestrator](https://github.com/ruskicoder/system-prompts).
It keeps Impeccable's design vocabulary for coding agents (steer with `polish` and `bolder`, review with
`critique`, `audit` and `a11y`, catch AI slop with the design hook) and changes three things:

- **Fully offline once installed.** No telemetry, no update checks, no hosted design catalog, no image API,
  no downloads. Every HTTP client in the engine refuses non-loopback hosts, and the headless browser runs
  behind a dead loopback proxy.
- **Host rules first.** When the agent runs under host rules for asking, reporting and scope (such as
  prompt-orchestrator's communication-tone), those rules win over the skill's own directives, and every
  change the skill makes on its own is reported.
- **WCAG 2.2 AA.** The `a11y` command audits and fixes against every Level A and AA criterion.

`FORK.md` lists every difference from upstream. The rest of this README is upstream's, adapted.

## Installation

procable is installed by prompt-orchestrator, user-wide, for every supported tool:

```bash
python3 install/install.py --yes                 # in a prompt-orchestrator checkout; procable is on by default
python3 install/install_procable.py status       # where it is installed and whether each copy has its engine
python3 install/install.py --uninstall-procable
```

The installer downloads the release pinned in `install/procable.lock.json` once, verifies every file
against `SHA256SUMS`, runs procable's own installer at user scope, and places the engine binary next to
each installed skill. For a machine without network, pass `--procable-bundle DIR` with a directory that
holds the release files (`SHA256SUMS`, `universal.zip`, and the engine for the platform).

Reload your coding tool afterwards, then run `/procable init` in your agent (`$procable init` on Codex).
In Codex, open `/hooks` once and approve the design hook.

## Why

Inter for everything. Purple-to-blue gradients. Cards nested in cards, gray text on colored backgrounds, a rounded-square icon tile above every heading. Ask an agent for a page with no other guidance and these show up again and again, across models and projects.

- **Words for what you want.** "Make it pop" leaves your agent to guess. `/procable bolder the pricing cards` loads a playbook written for that one move and points it at one target.
- **A reviewer.** `critique` scores the design against usability heuristics and ranks what to fix first. `audit` checks accessibility, performance, theming, and responsive behavior, with findings ranked P0 (blocking) to P3 (polish).
- **A slop detector.** 62 deterministic rules flag the tells above, plus quality problems like cramped padding and skipped headings. No LLM, no API key.

Procable is strongest on interfaces that already exist: point it at a page or a component and iterate. It builds new pages too; see [Build something new](https://impeccable.style/docs/new-work) for how to get the most out of that harder mode. To start, pick one component:

```
/procable critique the pricing cards
```

## How it works

Run `/procable init` once per project. It reads your code, asks only what it can't infer (who the product is for, what it needs to do, the constraints), and writes `PRODUCT.md`. Every later command reads it. Run `/procable document` to capture your existing visual system in `DESIGN.md`, so changes stay on-brand. [More on context files](https://impeccable.style/docs/context).

Then name a command and a target:

```
/procable critique the onboarding flow
/procable quieter the landing page
/procable polish settings
/procable harden checkout
```

Or just describe what you want: `/procable redo this hero section`. Narrow targets get better results; a single section reviewed closely beats a whole page at a glance. Pin a command you use often with `/procable pin audit` to get `/audit` as its own shortcut.

### 25 commands

| | Commands |
|---|---|
| **Steer** | `bolder` amplifies safe designs · `quieter` tones down loud ones · `distill` removes what the page doesn't need · `colorize` brings color to a gray interface · `typeset` fixes fonts, hierarchy, and sizing · `layout` fixes spacing and rhythm · `animate` adds transitions and interaction feedback · `delight` adds personality · `overdrive` goes for shaders, spring physics, and scroll-driven effects |
| **Review** | `critique` is a scored UX review · `audit` is a technical quality report · `a11y` is a WCAG 2.2 AA pass with fixes |
| **Finish** | `polish` is the final pass before shipping · `harden` covers errors, i18n, overflow, and edge cases · `clarify` rewrites unclear copy · `adapt` handles screen sizes and devices · `optimize` fixes loading and rendering performance · `onboard` designs first-run flows and empty states |
| **Iterate live** | `live` lets you pick an element in the browser and cycle through variants · `generate` produces variants of a named element without manual picking |
| **Plan and set up** | `shape` plans UX before code · `init` writes `PRODUCT.md` · `document` writes `DESIGN.md` · `extract` pulls repeated patterns into your design system |

One more, `craft`, is a deprecated alias for a plain build request. Type `/procable` alone to see the list in your agent. Each command has its own page: [impeccable.style/docs](https://impeccable.style/docs).

On Codex, open `/skills` or type `$procable` instead of the slash command.

### Live mode

`/procable live` opens your running dev server with an overlay. Select an element, pick a direction, and Procable hot-swaps variants in place. Accept one and it is written back to your source. It works with Vite, Next, SvelteKit, Astro, Nuxt, and plain HTML. [Iterate in the browser](https://impeccable.style/tutorials/iterate-live).

Live mode edits a local checkout only; don't inject it into a deployed site or weaken a production CSP to make it work. Applying copy edits runs your `package.json`'s optional `procable:manual-edit-validate` script with your permissions, so review it before using live mode in an unfamiliar repo.

## Three ways to run the detector

**In your agent.** The design hook scans every UI file your agent edits and hands the findings back before the agent calls the work done. This is where most slop gets caught: without the hook, the detector only runs when someone remembers to run it. It's available on Claude Code, Cursor, Codex, GitHub Copilot, Grok Build, and Hermes Agent, and the installer sets it up. Say yes. [Automatic design checks](https://impeccable.style/docs/hooks).

**In CI or your terminal.** The CLI needs no agent:

```bash
procable detect src/                   # scan a directory
procable detect http://localhost:5173      # scan a rendered local page (uses your installed Chrome or Edge)
procable detect --json .               # machine-readable output
procable detect --github src/          # GitHub Actions annotations
procable detect --list-rules           # every rule with its id and description
```

Source files like JSX and TSX get the pattern rules only. Rules that need a rendered page, such as low contrast and tiny text, run when you scan a URL, so point the CLI at your dev server too: `procable detect http://localhost:5173`.

Exit code `2` means findings, `1` means a target couldn't be scanned. To allow your brand font everywhere, run `procable ignores add-value overused-font Inter --reason "Brand font"`. To turn a rule off for one file, add a comment: `<!-- procable-disable overused-font: exported brand doc -->`. [Run a detector scan](https://impeccable.style/docs/detector) · [Configuration and ignores](https://impeccable.style/docs/config).

**In your browser.** procable ships no browser extension; run `procable detect` against your local dev server instead.

## Project files and git

Procable writes its working files under `.procable/`. A few are shared project artifacts and belong in the repo: `config.json`, `live/config.json`, `design.json`, `surfaces/*.md`, and `critique/*.md`. Keep `PRODUCT.md` and `DESIGN.md` in the repo too. The rest is screenshots, caches, and runtime state.

<details>
<summary><b>Copy the <code>.gitignore</code> entries</b></summary>

```gitignore
# procable-ignore-start
# Ephemeral output, runtime state, and per-dev overrides.
# The **/ prefix covers .procable at the repo root or in a nested workspace.
# Shared artifacts stay tracked: config.json, live/config.json,
# design.json, surfaces/*.md, critique/*.md.
**/.procable/config.local.json
**/.procable/hook.cache.json
**/.procable/hook.pending.json
**/.procable/*.png
**/.procable/review/
**/.procable/questions/
**/.procable/live/server.json
**/.procable/live/sessions/
**/.procable/live/previews/
**/.procable/live/annotations/
**/.procable/live/cache/
**/.procable/live/manual-edit-apply-transaction.json
**/.procable/live/manual-edit-events.jsonl
**/.procable/live/manual-edit-evidence/
**/.procable/live/pending-manual-edits.json
**/.procable/live/deferred-svelte-component-accepts.json
**/.procable/live/*.png
# procable-ignore-end
```

</details>

The folders that hold decision-round state (`.procable/questions/`, `.procable/mocks/worlds/`) also ignore themselves. A `.gitignore` can't untrack a file git already tracks; if you committed one earlier, run `git rm --cached <path>`.

## Good to know

**Build path.** For new landing pages and showcase surfaces, Procable can generate a full-fidelity comp first and build to match it (bolder, slower), or build straight in code (leaner, faster). App screens and documents are always built in code. `init` asks once and saves the answer as `buildPath` in `.procable/config.json`; the decision page lets you override it for one session. The choice only appears when your harness can generate images. [Image generation](https://impeccable.style/docs/image-generation) · [Configuration](https://impeccable.style/docs/config).

**Hooks run outside model approval.** In Claude Code, installed command hooks run even when you deny the model's own commands, so the first edit can download the engine. Review installed hooks before unattended runs, or pass `--settings '{"disableAllHooks": true}'` to turn them off for a run. The installer leaves hook entries it didn't add alone.

**Something broken?** `/procable doctor` finds stale context files, unused settings, and broken hooks. [Fix setup problems](https://impeccable.style/docs/doctor).

**Telemetry.** When a new design starts, the skill fetches a set of visual directions from the impeccable.style catalog. Turning telemetry off doesn't stop that request; it stops the anonymous record of which directions were shown and which one you picked, which is how the catalog learns what works. Your project name, files, code, and `PRODUCT.md` are never sent. Disable telemetry with `procable telemetry off`, `DO_NOT_TRACK=1`, or `"telemetry": false` in `.procable/config.json` for the whole project. The full list of what is sent is on the [privacy page](https://impeccable.style/privacy).

## Community

- [Issues](https://github.com/pbakaus/impeccable/issues) for bugs, questions, and ideas. Outside contributions start with an issue; see [DEVELOP.md](docs/DEVELOP.md).
- [Changelog](https://impeccable.style/changelog) for what shipped.
- Follow [@pbakaus](https://x.com/pbakaus) for release notes and new rules.

## License

Apache 2.0. See [LICENSE](LICENSE). Procable started from Anthropic's [frontend-design](https://github.com/anthropics/skills/tree/main/skills/frontend-design) skill.

---

Created by [Paul Bakaus](https://www.paulbakaus.com)
