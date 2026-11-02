# procable

procable is a modified fork of [Impeccable](https://github.com/pbakaus/impeccable) by Paul Bakaus,
maintained by Do Dang Khoa ([ruskicoder](https://github.com/ruskicoder)) for the
[prompt-orchestrator](https://github.com/ruskicoder/system-prompts) framework. The name stands for
process-ability, WCAG 2.2 AA skill sets, cleaned.

Upstream base: `pbakaus/impeccable` commit `e89e3ada` (skill 4.8.0, engine 0.1.18).
License: Apache-2.0, unchanged. Upstream copyright and notices are kept; see `NOTICE.md` and `LICENSE`.

## What differs from upstream

| Area | Upstream | procable |
|---|---|---|
| Network | update check, telemetry choice pings and the direction roll go to impeccable.style; card images load from it; installs and engine binaries download from GitHub; image generation calls OpenAI | runs fully offline once installed: one egress guard allows loopback only, and every upstream service call is removed |
| Telemetry | on by default, opt-out by env var | removed from the code |
| Direction deal | catalog roll from the hosted service | local degraded path only |
| Image generation | OpenAI API when a key is set | removed |
| Install and update | `impeccable install` and `update` fetch bundles | done by the prompt-orchestrator installer; engine network install paths are disabled |
| Engine binary | downloaded on first run | shipped next to the launcher by the installer; the launcher never downloads |
| Agent behaviour | skill directives such as "Finish the job. No hedging" | prompt-orchestrator `communication-tone` takes precedence over every skill directive; self-written detector ignores are always reported |
| Accessibility | general audit checks | adds a WCAG 2.2 AA reference and command |
| Name | `impeccable`, `IMPECCABLE_*`, `.impeccable/` | `procable`, `PROCABLE_*`, `.procable/`, applied by a re-runnable rename transform |
| Windows binary | signed with upstream's certificate | unsigned, published with its sha256 |

## Work stages

Each stage lands green in CI before the next one starts.

1. Fork set-up: rename of the repository, this file, notices, contributor rules.
2. Offline: egress guard, removal of telemetry, update check, roll API, card images, image generation and install downloads; offline launcher and shim tests.
3. Behaviour: skill text aligned with `communication-tone` (directives D1 to D8 in the vetting notes).
4. Release: engine 0.2.0 built in CI for every target (Windows unsigned), no npm packages, `release-procable` publishes engines plus the skill bundle.
5. WCAG 2.2 AA: the `a11y` command and `reference/a11y.md`.
6. Rename to procable with `scripts/fork/rename-procable.mjs`; product version 1.0.0.
7. prompt-orchestrator integration: `install/install_procable.py` in system-prompts installs procable user-wide for every supported tool.

## Syncing upstream

1. `git fetch upstream` and merge the wanted upstream commit into a branch.
2. Re-run `node scripts/fork/rename-procable.mjs --apply` on the merge result, then dispatch `record-oracle` (with `bundle: true` when the detector changed).
3. Re-vet every changed directive and every new network call before merging to `main`.
4. Record the new upstream base commit in this file.
