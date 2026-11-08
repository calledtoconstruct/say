# Guiding principles: calledtoconstruct/say

## Purpose and scope
Say: a native picture talker for children aged 2 and up, with a parent setup, built on GTK and the Tobii gaze daemon.

## Stack and layout
Rust 2021 crate `say` (library `src/lib.rs` plus binary), GTK4 and cairo. Modules in `src/` (board, gaze, layout, predict, speak, store, llama and others). Word files and pictures live in `content/` (one JSON file and one picture per word; credits in `content/SOURCES.txt`).

## Commands
| Check | Command | Evidence |
|---|---|---|
| Build / run | `cargo run --release`; parent setup `cargo run --release -- --setup` | README.md |
| Self-check | `cargo run --release -- --selfcheck` | README.md |
| Type-check | `cargo check` (standard Cargo; not written in the README) | Cargo.toml |
| Tests | No test files found; `cargo test` will run any unit tests added in modules | repository tree |
| CI | None | repository tree |

Commands marked as not written down come from the build files, not from project docs; confirm them before relying on them.

## Before every push
Before every push: run the build and type-check/lint that exist here, plus only the tests for the files or feature you changed (team rule, ideas #14 note 6797). Do not run the full suite locally as a routine gate; this repository has no CI that runs the full suite, so run the full suite only when the change is broad, and say in the PR that CI coverage is missing. Merge needs a positive review. Explain any new failure your change caused. Never weaken a valid test to make it pass, and never invent a command this file does not list.

## Project rules
- The child uses the board; only the parent changes stage and skill. The board never changes stage by itself.
- Prediction stays on the computer (word graph); the llama.cpp option is an opt-in experiment.
- Every symbol or photo added to `content/` gets its credit and license in SOURCES.txt.

## What to avoid
- Committing to the default branch; work on a branch and open a pull request.
- Deleting files, branches or history, or changing repository settings.
- Adding tooling, CI or dependencies that the change does not need.
- Rules copied from other projects (file-size limits, clocks, quotas) that this repository never adopted.

## Delivery and safety
- Not published (`publish = false`).

## Open questions
- No automated tests or CI; `--selfcheck` is the closest check.

## Sources
Read from the `main` branch on 2026-11-08 ET: `README.md`, `Cargo.toml`, repository tree.

## Instruction updates
Check `guiding-principles.version` at the start of each turn and when you enter this project; reload this file when the version changes. Change this file, its version and date, and `PRINCIPLES_HISTORY.md` in the same commit. Team-wide role instructions are versioned separately in the team-process repository.
