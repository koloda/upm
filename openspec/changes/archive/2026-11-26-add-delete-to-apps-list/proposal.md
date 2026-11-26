# Proposal

## Why

UPM can inventory, check and upgrade installed software, but it cannot remove it: the `Source` contract exposes only `inventory`, `outdated` and `upgrade`, so users must leave UPM and find each manager's uninstall command by hand. The graphical apps list shows every installed item yet offers no delete action, which makes UPM a read-mostly viewer for software it cannot manage.

## What Changes

- Add a removal operation to the core `Source` contract, implemented by every source whose own manager can uninstall what it installed; sources whose items are plain files or firmware have no removal and report that explicitly.
- Add a targeted-only removal operation to the engine that resolves the selected rows, runs uninstalls with the usual sudo, dry-run and interruption handling, and returns a summary.
- Build a removal plan from the GUI selection: empty or stale selections remove nothing, and the manual source is never removable.
- Add a **Delete** button to each apps-list row in the GUI, with a confirmation dialog naming the exact items, a "no removable items" state, busy/close protection, log output and a refresh of the affected sources afterwards.
- Add QML tests and core unit tests for the new behavior. The CLI is unchanged: there is no new command-line verb in this change.

## Capabilities

### New Capabilities
- `app-deletion`: End-to-end removal behavior — which sources can uninstall, targeted-only selection, confirmation, dry-run, sudo, interruption, result reporting and post-removal refresh.

### Modified Capabilities
- `source-registry`: The common source contract gains an optional removal hook and a removal-feasibility declaration; adding a removable source must still require no core changes.
- `gui-frontend`: The package table gains a per-row Delete control and a deletion confirmation; busy and close protection now cover deletion as well as upgrade.
- `acceptance-tests`: The QML interface suite gains deletion scenarios; the black-box CLI suite is unchanged.

## Impact

- `common/src/source.rs` (`Source` trait), `common/src/engine.rs`, `common/src/catalog.rs`, `common/src/sources/*.rs` (per-source uninstall commands).
- `gui/src/backend.rs`, `gui/src/jobs.rs`, `gui/src/view.rs`, `gui/qml/MainView.qml`.
- `gui/tests/tst_mainview.qml`, plus Rust unit tests in `common` and `gui`.
- No new dependencies, no data migration, no CLI changes. Removal is destructive, so safety and confirmation are the main risks.
