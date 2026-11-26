## Purpose

`upm-gui` is the QML desktop front end. It shows the same catalog as the CLI as a window: a source sidebar, a searchable and filterable package table with checkboxes, a Refresh action, an Upgrade button that asks for confirmation, and a log panel. Long work runs on a background thread so the window stays responsive. This capability describes the window's observable behaviour and the contract between QML and the Rust backend.

## Requirements

### Requirement: Startup
The system MUST refuse to start with superuser privileges, and on start SHALL load the installed inventory and show a hint to press Refresh to check for updates.

**Source:** gui/src/main.rs:10, gui/src/jobs.rs:44

#### Scenario: Load on start
- **GIVEN** the window opens
- **WHEN** loading finishes
- **THEN** the catalog is filled and the status suggests pressing Refresh

#### Scenario: Root is refused
- **GIVEN** the GUI is started as root
- **WHEN** it runs
- **THEN** it exits without opening the window

### Requirement: Source sidebar
The system SHALL show an "All" entry with totals followed by one entry per source, each with its installed count, an update badge and a failure marker, and selecting an entry MUST filter the table to that source.

**Source:** gui/src/view.rs:29, gui/qml/MainView.qml:95

#### Scenario: Sidebar totals
- **GIVEN** a populated catalog
- **WHEN** the sidebar is shown
- **THEN** the first entry is "All" with the summed installed and update counts

#### Scenario: Selecting a source filters rows
- **GIVEN** the sidebar
- **WHEN** the user picks a source
- **THEN** the table shows only that source's packages

### Requirement: Search and upgrade filter
The system SHALL filter the table by a search text and by a "only with updates" checkbox, and the checkbox MUST follow backend-driven changes to the filter.

**Source:** gui/qml/MainView.qml:158

#### Scenario: Search filters rows
- **GIVEN** the table is showing packages
- **WHEN** the user types `cod` in the search field
- **THEN** the backend is asked to filter by `cod`

#### Scenario: Checkbox re-binds after a click
- **GIVEN** the "only with updates" checkbox
- **WHEN** the user clicks it and the backend later changes the flag
- **THEN** the checkbox reflects the backend's value

### Requirement: Package table and selection
The system SHALL show one row per package with its installed version, the new version (or a state word such as "current", "manual" or "not checked"), and its source, and SHALL allow selecting rows except manual ones.

**Source:** gui/qml/MainView.qml:214, gui/qml/MainView.qml:55

#### Scenario: State words reflect package state
- **GIVEN** an upgradable, a current and a manual row
- **WHEN** the table is shown
- **THEN** they read the new version, "актуальний", and "вручну" respectively

#### Scenario: Manual rows cannot be selected
- **GIVEN** a manual package row
- **WHEN** the user tries to select it
- **THEN** it cannot be selected

#### Scenario: Select-all skips manual
- **GIVEN** a table containing manual and non-manual rows
- **WHEN** the user uses select-all
- **THEN** only the non-manual rows are selected

### Requirement: Upgrade confirmation
The system SHALL ask for confirmation before upgrading, describing what will happen (the selected packages, everything in the current source, or everything), and MUST forbid confirming a plan that has nothing to run.

**Source:** gui/src/view.rs:73, gui/qml/MainView.qml:330

#### Scenario: Selection confirmation names the packages
- **GIVEN** selected packages
- **WHEN** the user presses Upgrade
- **THEN** a dialog titled "Оновити вибрані пакунки?" lists them

#### Scenario: Nothing to run cannot be confirmed
- **GIVEN** a plan with nothing to run
- **WHEN** the dialog opens
- **THEN** its confirm button is disabled

#### Scenario: Confirming clears the selection
- **GIVEN** an open confirmation for selected packages
- **WHEN** the user confirms
- **THEN** the upgrade starts and the table selection is cleared

### Requirement: Busy and close protection
The system SHALL allow only one background job at a time, disable Refresh, Upgrade and Delete while busy, and MUST NOT let the window close while an upgrade or a deletion is in progress (warning the user instead).

**Source:** gui/src/backend.rs:169, gui/qml/MainView.qml:31

#### Scenario: Busy blocks actions
- **GIVEN** a job is running
- **WHEN** the user looks at the controls
- **THEN** Refresh, Upgrade and Delete are disabled

#### Scenario: Close is blocked during upgrade
- **GIVEN** an upgrade is in progress
- **WHEN** the user tries to close the window
- **THEN** closing is refused and a warning is shown

#### Scenario: Close is blocked during deletion
- **GIVEN** a deletion is in progress
- **WHEN** the user tries to close the window
- **THEN** closing is refused and a warning is shown

### Requirement: Log panel
The system SHALL show a log panel with the running command output, open it automatically when an upgrade starts, and keep the newest line in view.

**Source:** gui/qml/MainView.qml:74, gui/src/backend.rs:152

#### Scenario: Log opens on upgrade
- **GIVEN** an upgrade starts
- **WHEN** the upgrading flag turns on
- **THEN** the log panel becomes visible

### Requirement: QML/Rust contract
The system SHALL pass the sidebar, rows and confirmation plan to QML as JSON, and MUST treat a malformed selection payload as invalid rather than as an empty (meaning "everything") selection.

**Source:** gui/src/view.rs:105, gui/src/backend.rs:230

#### Scenario: Broken selection is not empty
- **GIVEN** a selection payload that is not valid JSON
- **WHEN** it is parsed
- **THEN** it is rejected and no upgrade is started

### Requirement: Deletion availability lives in the action bar, not the rows
The system MUST NOT show a Delete control on individual package rows. Each row SHALL only expose whether its source can uninstall it so that the bottom action bar can decide which selected items are removable.

**Source:** gui/qml/MainView.qml:244, gui/src/view.rs:45

#### Scenario: Rows have no per-item Delete button
- **GIVEN** the apps list with removable, unsupported and manual rows
- **WHEN** the user looks at a row's controls
- **THEN** no Delete button is offered on that row

#### Scenario: Removability still gates the bottom control
- **GIVEN** a removable row and an unsupported or manual row
- **WHEN** the user selects rows
- **THEN** only removable selections enable the bottom Delete selected control

### Requirement: Bulk delete control in the action bar
The system SHALL show a Delete selected control next to the Upgrade selected control at the bottom of the apps list, showing the count of selected removable items, and MUST disable it while a background job is running or when no selected item can be removed.

**Source:** gui/qml/MainView.qml:27, gui/qml/MainView.qml:339

#### Scenario: Delete selected becomes available with removable selection
- **GIVEN** one or more selected rows from sources with a native uninstall command
- **WHEN** the user looks at the bottom action bar
- **THEN** a Delete selected control is enabled and shows how many removable items are selected

#### Scenario: Delete selected is unavailable without removable selection
- **GIVEN** a selection containing only items whose source cannot uninstall them, or no selection at all
- **WHEN** the user looks at the bottom action bar
- **THEN** the Delete selected control is disabled

#### Scenario: Delete selected uses the same confirmation flow
- **GIVEN** a selection containing removable items
- **WHEN** the user presses the Delete selected control
- **THEN** the delete confirmation dialog opens for that selection

### Requirement: Selected rows are visually marked
The system SHALL give each selected package row an accent highlight so the current selection is distinguishable from unselected rows, and MUST remove the highlight when the selection is cleared.

**Source:** gui/qml/MainView.qml:244

#### Scenario: Selecting a row highlights it
- **GIVEN** an unselected package row
- **WHEN** the user selects it
- **THEN** the row is shown with the accent highlight

#### Scenario: Deselecting a row removes the highlight
- **GIVEN** a highlighted selected row
- **WHEN** the user deselects it
- **THEN** the row returns to its ordinary appearance

### Requirement: Delete confirmation
The system SHALL ask for confirmation before deleting, listing each item and its source, MUST forbid confirming when nothing is removable, and MUST start the deletion and clear the selection only when the user confirms.

**Source:** gui/qml/MainView.qml:351, gui/src/view.rs:73

#### Scenario: Delete confirmation names the items
- **GIVEN** one or more selected removable packages
- **WHEN** the user presses Delete
- **THEN** a confirmation dialog lists the selected packages with their sources

#### Scenario: Confirming deletion clears the selection
- **GIVEN** an open deletion confirmation for a valid selection
- **WHEN** the user confirms it
- **THEN** the deletion starts and the table selection is cleared

### Requirement: Delete progress and result
The system SHALL open the log panel when deletion starts, show the uninstall commands and their result, and report removed, skipped and failed items in the status area.

**Source:** gui/src/jobs.rs:37, gui/qml/MainView.qml:285

#### Scenario: Log opens on deletion
- **GIVEN** a deletion is started
- **WHEN** the busy flag turns on
- **THEN** the log panel becomes visible and shows the removal output

#### Scenario: Failures are visible in the status
- **GIVEN** a deletion where one uninstall failed
- **WHEN** the job finishes
- **THEN** the status reports the failure and the failed command appears in the log

<!-- __inferspec_meta__: {"hash": "c0ad22bfac0995aa32b1f6222e731a5d3f7b0748e25b687745fcdbd946fe683b", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
