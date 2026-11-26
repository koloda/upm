# Spec Delta

## ADDED Requirements

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

## MODIFIED Requirements

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
