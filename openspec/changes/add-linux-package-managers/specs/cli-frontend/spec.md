# Spec Delta

## ADDED Requirements

### Requirement: Only relevant sources are reported
The CLI MUST NOT show a source that is absent from this computer unless the user names it.

#### Scenario: Ubuntu update
- **GIVEN** an Ubuntu host
- **WHEN** `update` runs
- **THEN** there is an APT section and no DNF, Pacman, Zypper or APK section

#### Scenario: Named absent source
- **GIVEN** an Ubuntu host
- **WHEN** `update dnf` runs
- **THEN** the DNF section says the source is not available here

### Requirement: Skipped work is explained
After `upgrade`, the CLI SHALL show the notes explaining why a source was skipped.

#### Scenario: Selected Pacman package
- **GIVEN** an Arch host
- **WHEN** `upgrade pacman:firefox` runs
- **THEN** the summary lists pacman as skipped with a note to upgrade the whole source
