# Runtime environment the binary reads (context crate)

The JS scripts learned two things from their own file location that a single
binary cannot: which harness built them (`lib/provider.mjs`) and where the
skill's `reference/` and `SKILL.md` live (`../reference`, `../SKILL.md`).
`crates/context/src/provider.rs` resolves both at run time.

| Variable | Meaning | Default |
|---|---|---|
| `PROCABLE_SKILL_DIR` | The skill directory (holds `SKILL.md`, `reference/`, `scripts/`). Used for the native platform references `context` inlines, `pin`'s `command-metadata.json`, and `concept-seed`'s default catalog dir (`<skill>/scripts`). | Walk up from the executable's directory until a directory containing `reference/ios.md` is found (the binary ships at `<skill>/scripts/bin/<os>-<arch>/`). None if nothing matches: native refs are then skipped silently, like a missing file in the JS. |
| `PROCABLE_PROVIDER_ID` | The build provider id (`claude-code`, `codex`, `cursor`, ...). Selects the hook manifest paths `context` and `doctor` inspect and the `$`/`/` command prefix (`$` only for `codex`). | Derived from the skill dir's harness folder (`<root>/.codex/skills/procable` -> `codex`); otherwise `source`, which is what the JS reads in a source checkout. |
| `PROCABLE_SELF` | How to spell this binary in printed commands where the JS printed `node <scripts>/<script>.mjs` (`context`'s MANUAL_DETECTOR_REQUIRED, SURFACE_CONTEXT_AVAILABLE, MONOREPO_TARGET_REQUIRED, `serve-question --start`'s follow-up line). Printed as `<self> <verb>`; a Windows `.cmd` path that has to be quoted (one containing spaces, or any quoted hook spelling) is printed as `& '<path>' <verb>` (a PowerShell literal, `'` doubled) so PowerShell runs it (`procable_common::launcher`, #744). | The executable path. |
| `PROCABLE_SEED_SALT` | Pins the salt seed references are computed under (`procable_common::seed`), so tests and the oracle print stable references. Never set it in normal use: the per-user salt is what keeps a committed reference from being computed out of a key. | The contents of `~/.procable/seed-salt`, 128 random bits created on first use. |

Everything else is the JS contract's own environment (`PROCABLE_CONTEXT_DIR`,
`PROCABLE_STALENESS_CACHE`, `PROCABLE_CATALOG_DIR`, `PROCABLE_IMAGE_GEN_FAKE`,
...), read unchanged. procable removed the network variables; see the procable
deltas in `docs/CLI-CONTRACT.md`.

The launcher (`launcher/procable`) does not export these yet; a launcher that
sets `PROCABLE_SKILL_DIR="$dir/.."` and `PROCABLE_SELF="$0"` would make
directives name the launcher instead of the platform binary.

Oracle note: run the public repo's `tests/oracle/run.mjs` with
`PROCABLE_SKILL_DIR=<public repo>/skill` (the corpus expects the native
references inlined) and an `PROCABLE_BIN` path outside `$HOME` (the
harness normalizes `$HOME` before it normalizes the bin path, so a binary
under the home dir renders as `<HOME>/...` instead of `<PROCABLE>`).
