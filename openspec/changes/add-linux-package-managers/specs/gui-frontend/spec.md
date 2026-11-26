# Spec Delta

## MODIFIED Requirements

### Requirement: Source sidebar
The system SHALL show an "All" entry with totals followed by one entry per source present on this computer, each with its installed count, an update badge and a failure marker, and selecting an entry MUST filter the table to that source. Sources absent from this computer, including other distribution families' native sources, MUST NOT appear.

**Source:** gui/src/view.rs:29, gui/qml/MainView.qml:95

#### Scenario: Sidebar totals
- **GIVEN** a populated catalog
- **WHEN** the sidebar is shown
- **THEN** the first entry is "All" with the summed installed and update counts

#### Scenario: Selecting a source filters rows
- **GIVEN** the sidebar
- **WHEN** the user picks a source
- **THEN** the table shows only that source's packages

#### Scenario: Debian host
- **GIVEN** the window opens on a Debian host
- **WHEN** loading finishes
- **THEN** the sidebar has an APT entry and no DNF, YUM, Zypper, RPM, Pacman, AUR or APK entry

#### Scenario: Fedora host
- **GIVEN** the window opens on a Fedora host
- **WHEN** loading finishes
- **THEN** the sidebar has a DNF entry and no APT or Pacman entry

## ADDED Requirements

### Requirement: Skipped work is visible in the status
When an upgrade or removal skips a source, the status area MUST name the skipped source instead of reporting plain completion, and the log MUST show the explaining note.

#### Scenario: Selected Pacman package
- **GIVEN** a selected Pacman package
- **WHEN** the user confirms the upgrade
- **THEN** the status says Pacman was skipped and the log explains why

#### Scenario: Nothing skipped
- **GIVEN** an upgrade where every source succeeds
- **WHEN** it finishes
- **THEN** the status reports completion
