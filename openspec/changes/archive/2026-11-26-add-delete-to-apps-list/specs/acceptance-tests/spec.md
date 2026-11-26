# Spec Delta

## ADDED Requirements

### Requirement: Deletion interface tests
The QML suite SHALL drive the delete control against a fake backend, covering its presence and enabled state, the confirmation flow, cancellation, the empty and manual cases, busy blocking and the post-deletion refresh; core unit tests SHALL cover the removal plan and summary.

**Source:** gui/tests/tst_mainview.qml:98, common/src/catalog.rs:199

#### Scenario: Delete asks for confirmation and uses the same selection
- **GIVEN** a selected removable package and a pressed Delete button
- **WHEN** the dialog is shown
- **THEN** the backend is asked to plan the deletion and confirming asks it to delete with the same selection

#### Scenario: Cancel does not delete
- **GIVEN** a deletion confirmation
- **WHEN** the user cancels it
- **THEN** the backend is not asked to delete

### Requirement: Unchanged CLI acceptance contract
The black-box CLI suite MUST continue to assert the `list`, `update` and `upgrade` behavior only, and MUST NOT require a delete command or gain new mutating commands.

**Source:** tests/blackbox.sh:174

#### Scenario: CLI surface is unchanged
- **GIVEN** the black-box suite
- **WHEN** it runs against the stubbed managers
- **THEN** it exercises only the existing command-line verbs
