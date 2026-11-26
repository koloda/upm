# Design

## Context

UPM's core returns data and never prints; both frontends render it. The `Source` trait (`common/src/source.rs:155`) has `inventory`/`outdated`/`upgrade`/`needs_sudo`, the engine owns `resolve`/`list`/`check_updates`/`upgrade` and keeps sudo alive with a background `sudo -n -v` thread, the catalog turns a selection into an `upgrade_plan`, and the GUI sends selections to the Rust backend as JSON with `Job`s running on one background thread. The GUI row delegate is the apps list; manual rows are deliberately not selectable. See `proposal.md` for motivation and the delta specs for required behavior.

## Goals / Non-Goals

**Goals:**
- Reuse the existing source/engine/catalog/backend pipeline for removal instead of building a second path.
- Make removal explicit and auditable: only selected rows, always confirmed, all commands logged.
- Keep the window responsive and the inventory correct after removal.

**Non-Goals:**
- No CLI delete verb and no changes to the black-box CLI suite.
- No file deletion for sources without a native uninstall, and no firmware removal.
- No undo/restore, no bulk delete-all, no translation of the existing specs.

## Decisions

**1. Extend `Source` with a defaulted removal hook and a feasibility flag.** `fn remove(&self, ctx, pick) -> Status` defaults to `Status::Skipped`, and `fn can_remove(&self) -> bool` defaults to `false`; only feasible sources override them. Rationale: the contract stays cheap — existing sources compile unchanged, and "no removal" is explicit rather than an error. Alternative considered: a separate `Removable` trait — rejected because it duplicates registry iteration and would make the GUI ask two questions per row.

**2. Feasible set = "own manager can uninstall".** Implement removal for `dnf` (`sudo dnf remove` with the same sudo handling as upgrade), `flatpak` (`flatpak uninstall` per installation), `brew` (`brew uninstall`), `appimage` (the `app-manager` removal subcommand), `npm` (`npm uninstall -g --prefix`, plus the pnpm/bun mirrors), `composer` (`composer global remove`), `rust` (`rustup toolchain uninstall` / `cargo uninstall`), `python` (`pip uninstall`, `uv tool uninstall`, `pipx uninstall`), `sdkman` (`sdk uninstall`), `android` (`sdkmanager --uninstall`) and `shell` (`fisher remove`, `gh extension remove`). Exclude `go`, `tools`, `firmware` and `manual`: their items are files or firmware with no uninstall command owned by UPM. The exact subcommand spelling for `appimage`, `sdkman` and `android` MUST be confirmed against each tool's `--help` during implementation and pinned in the stubbed tests. Rationale: the user chose "uninstall via source manager" for all feasible sources; treating file deletion as uninstall would be unsafe.

**3. Engine operation mirrors upgrade but is strictly targeted.** `engine::remove(ctx, scope, picks, sources)` reuses `resolve` and the existing sudo-keepalive/interruption logic, returns a `Summary`, and accepts no "everything" scope from the GUI. Rationale: an accidental global delete is the highest-risk failure; whole-source and global deletion are not part of this change. Alternative considered: reusing `upgrade` with a flag — rejected because its scope semantics deliberately allow "all sources".

**4. Catalog produces a `deletion_plan` rather than reusing `upgrade_plan`.** It maps the same `(source, key)` selection to targeted picks, drops stale keys and non-removable sources/items, and exposes `runnable` plus a per-row `removable` flag in the row JSON. Rationale: upgrade's plan falls back to "all" when selection is empty — exactly the behavior deletion must not have.

**5. GUI reuses the existing confirmation dialog and job loop.** Add `Backend::plan_delete`/`Backend::delete_selected` invokables (the Rust method cannot be named `delete` because `delete` is a C++ keyword in the generated cxx-qt header; the QML names are `planDelete`/`deleteSelected`), a `Job::Delete { source, selected }`, a `view::delete_confirmation` JSON payload and a row Delete button. The dialog is parameterized by mode (upgrade/delete) rather than duplicated. On finish, only the touched sources are reloaded, the selection is cleared and the log panel opens through the existing `upgrading`/busy signal. Rationale: one background job, one log, one close guard.

**6. Dry-run and yes are already on `Context`.** The GUI constructs `Context` with `yes = true` and derives `dry` from argv; the removal job passes the same context, so uninstall commands are printed but not executed under `-n`, and no reload is needed for a dry run.

## Risks / Trade-offs

- [Destructive action with no undo] → Explicit row selection, a confirmation naming every item, and no bulk delete path.
- [Wrong uninstall command for a tool] → Verify each command against `--help` and lock it into unit/stub tests before enabling the source.
- [Partial removal across many sources] → Per-item status and failed-command logging; reload only touched sources so the surviving inventory stays accurate.
- [Removal while window closes] → Reuse the existing busy/close guard; extend it to the delete job.
- [Manual and self-updating apps look removable but are not] → Per-row `removable` flag disables the control and the tooltip explains why; file deletion is never performed.

## Migration Plan

No data migration: removal is a new, opt-in operation and existing inventory, checks and upgrades are unchanged. Rollback is to leave the feature disabled at build time (no source overrides the default removal hook).

## Open Questions

- Should a future change expose removal through the CLI (`upm delete <target>`) and add a black-box suite for it? Out of scope here; answering it later does not alter this design.
