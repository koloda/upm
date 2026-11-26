# Tasks

## 1. Core contract and source implementations

- [x] 1.1 Add `can_remove() -> bool` (default `false`) and `remove(&mut Context, Pick) -> Status` (default `Status::Skipped`) to the `Source` trait in `common/src/source.rs`, and verify `cargo check -p upm-common` builds with every existing source unchanged.
- [x] 1.2 Implement removal for `dnf` (`sudo dnf remove` with selectable `-y`, reusing the existing sudo hint) and verify a unit test asserts the exact command and selected keys.
- [x] 1.3 Implement removal for `flatpak` (`flatpak uninstall`, splitting the `installation app` key into the correct installation flag), and verify a unit test asserts user and system uninstalls are not mixed.
- [x] 1.4 Implement removal for `brew`, `appimage` and confirm the `app-manager` removal subcommand against `app-manager --help`; verify a unit test asserts one uninstall per selected key.
- [x] 1.5 Implement removal for `npm` (each prefix, plus the pnpm/bun mirrors), `composer` (`composer global remove`) and `rust` (`rustup toolchain uninstall` / `cargo uninstall`); verify unit tests pin each command shape.
- [x] 1.6 Implement removal for `python` (`pip uninstall`, `uv tool uninstall`, `pipx uninstall`), skipping keys that belong to a removed Python, and verify a unit test covers each key prefix.
- [x] 1.7 Implement removal for `sdkman`, `android` (`sdkmanager --uninstall`) and `shell` (`fisher remove`, `gh extension remove`), confirming each subcommand against `--help`; verify unit tests assert the exact commands.
- [x] 1.8 Verify `go`, `tools`, `firmware` and `manual` keep the default "not removable" behavior, and assert a unit test shows no command is run and no file is deleted for them.

## 2. Engine and catalog

- [x] 2.1 Add a removal operation to `common/src/engine.rs` that resolves selected keys, applies sudo keepalive once when needed, honours the cancel flag and returns a `Summary`, and verify a unit test covers success, skip, failure and interruption.
- [x] 2.2 Make the removal operation refuse an empty or stale selection and never expand to whole-source or global removal, and verify a unit test shows zero commands are run for those cases.
- [x] 2.3 Add a targeted-only `deletion_plan` to `common/src/catalog.rs` that drops stale and non-removable items, exposes `runnable` and a per-row `removable` flag, and verify unit tests cover a valid selection, a stale selection and a manual item.

## 3. GUI backend

- [x] 3.1 Emit the `removable` flag in the row JSON and add a `view::delete_confirmation` payload with title, lines and `runnable`, and verify `cargo test -p upm-gui` covers a removable item, an unsupported item and malformed input.
- [x] 3.2 Add `plan_delete` and `delete_selected` invokables (QML: `planDelete` / `deleteSelected`; `delete` is a C++ keyword) to `gui/src/backend.rs` that reject a malformed selection and clear the selection on confirm, and verify a backend unit test distinguishes `[]` from broken JSON.
- [x] 3.3 Add `Job::Delete` to `gui/src/jobs.rs` so it runs the engine removal, reports failures through the `Journal`, reloads only the touched sources and skips the reload in dry-run mode, and verify a unit test covers the reload set.
- [x] 3.4 Wire the deletion job into the existing busy/close guard and log-panel signal, and verify a backend test shows a second job is refused while one is running.

## 4. QML interface

- [x] 4.1 Keep deletion out of individual rows: the `MainView.qml` delegate exposes only whether its source is removable, and the QML test verifies no per-row Delete button exists.
- [x] 4.2 Parameterize the confirmation dialog for deletion so it lists each item and source and disables confirm when `runnable` is false, and verify a QML test covers the enabled and disabled states.
- [x] 4.3 Wire the Delete button to `plan_delete` (`planDelete`) and the confirmed dialog to `delete_selected` (`deleteSelected`), clearing the selection on accept and leaving it intact on cancel, and verify a QML test asserts the backend calls and selection state.
- [x] 4.4 Add a `deleteSelectedButton` in the bottom action bar next to `upgradeButton`, labelled `Видалити вибрані` with the removable-selection count, enabled only when the selection contains removable items and no job is busy, and wired to the same `askDelete()` confirmation flow.
- [x] 4.5 Mark the selected package row with an accent highlight (`highlighted: root.isSelected(modelData)`), clearing it on deselection, and verify a QML test toggles the highlight.

## 5. Verification

- [x] 5.1 Extend `gui/tests/tst_mainview.qml` with deletion scenarios: presence/enabled state, confirmation flow, cancel, empty plan, manual restriction, busy blocking and post-deletion refresh, and verify `QT_QPA_PLATFORM=offscreen qmltestrunner -input gui/tests` passes.
- [x] 5.2 Add Rust unit tests for the per-source commands, deletion plan, summary and dry-run behavior, and verify `cargo test --workspace` passes.
- [x] 5.3 Confirm the CLI surface is unchanged by running `tests/blackbox.sh` and verifying it still passes without any delete command or new mutating command.
- [x] 5.4 Cover the bottom Delete selected control with QML tests for label/count, removable-only enabling, busy blocking and the shared confirmation flow, and verify `qmltestrunner -input gui/tests` passes.
- [x] 5.6 Cover the selected-row highlight with a QML test that toggles selection on and off, and verify `qmltestrunner -input gui/tests` passes.
- [x] 5.5 Elevate npm uninstalls whose prefix is not writable (as upgrades already do), and verify a unit test shows `sudo npm uninstall -g --prefix <prefix>` for a root-owned prefix while writable prefixes stay unprivileged.
