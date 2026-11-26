## Purpose

UPM is verified at two levels: a black-box suite that runs the real CLI against stubbed package managers and a fake home directory, asserting exactly which commands are called and what is printed; and QML tests that drive the window against a fake backend. Together they are the executable acceptance contract for the product.

## Requirements

### Requirement: Black-box CLI coverage
The system SHALL provide a black-box suite that runs the CLI with a fake home directory and stub package managers, and asserts command-line output and the exact commands invoked, without modifying the real system.

**Source:** tests/blackbox.sh:5, tests/blackbox.sh:11

#### Scenario: Managers are stubbed
- **GIVEN** the black-box suite runs
- **WHEN** the CLI invokes a package manager
- **THEN** the recorded call comes from the stub, not the real manager

#### Scenario: Read-only commands do not mutate
- **GIVEN** a read-only command such as `list` or `update`
- **WHEN** it runs under the suite
- **THEN** no mutating command is invoked

### Requirement: Behavioural assertions
The suite SHALL assert the behaviour of `list` (including filters, all, dependency search, substring and `source:package`), of `update` (including rebuild detection and dry-run), and of `upgrade` (including per-source, targeted, auto-yes, dry-run and the legacy option-only form).

**Source:** tests/blackbox.sh:174, tests/blackbox.sh:192, tests/blackbox.sh:206

#### Scenario: Targeted upgrade only touches the target
- **GIVEN** an upgrade of a specific package
- **WHEN** it runs under the suite
- **THEN** only that package's update command is invoked

#### Scenario: Legacy option-only form upgrades
- **GIVEN** the invocation `--only npm -n`
- **WHEN** it runs under the suite
- **THEN** it behaves as an upgrade limited to npm

### Requirement: Interruption and robustness assertions
The suite SHALL assert that an interrupt during an upgrade yields exit code 130 with a summary, that an interrupt during an update stops further queries, and that a closed output pipe does not crash the program.

**Source:** tests/blackbox.sh:235, tests/blackbox.sh:238

#### Scenario: Interrupted upgrade exits 130
- **GIVEN** an interrupt is delivered during an upgrade
- **WHEN** the CLI finishes
- **THEN** it prints a summary and exits 130

#### Scenario: Closed pipe does not panic
- **GIVEN** the output pipe is closed early
- **WHEN** the CLI keeps writing
- **THEN** it exits without panicking

### Requirement: QML interface tests
The system SHALL provide QML tests that drive the window against a fake backend, covering sidebar and row counts, source selection, state texts, the upgrade button and confirmation flow, busy blocking, filter re-binding, the manual-source restriction and close protection.

**Source:** gui/tests/tst_mainview.qml:98, gui/tests/tst_mainview.qml:186

#### Scenario: Upgrade needs confirmation
- **GIVEN** a selected package and a pressed Upgrade button
- **WHEN** the dialog is shown
- **THEN** the backend is asked to plan, and confirming asks it to upgrade with the same selection

#### Scenario: Manual source cannot be upgraded
- **GIVEN** the manual source is current
- **WHEN** the interface is examined
- **THEN** the Upgrade button is disabled

<!-- __inferspec_meta__: {"hash": "3f26660d6c4d5ac135075a9aabe11a1ca87c76abbc014f22d348db9dd1c9b5b5", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
