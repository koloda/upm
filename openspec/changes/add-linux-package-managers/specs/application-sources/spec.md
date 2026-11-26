# Spec Delta

## MODIFIED Requirements

### Requirement: Self-updating programs
The system SHALL track a known set of separately installed programs, listing only those that are present and not owned by the distribution's package manager, reading their version, checking for updates through the program's own check command, and updating through the program's own updater.

**Source:** common/src/sources/apps.rs:151

#### Scenario: Only present, non-RPM programs are listed
- **GIVEN** a known program that is absent, and one that is present but owned by a distribution package
- **WHEN** the tools source lists
- **THEN** neither is listed

#### Scenario: Update availability is parsed
- **GIVEN** a tool whose check output says an update is available
- **WHEN** the tools source checks for updates
- **THEN** the newer version is reported

#### Scenario: Program without a check only updates on upgrade
- **GIVEN** a tracked program that cannot check for updates without updating
- **WHEN** updates are checked
- **THEN** it is reported as unchecked rather than as up to date
