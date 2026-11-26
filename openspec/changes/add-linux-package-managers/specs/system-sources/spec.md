# Spec Delta

## MODIFIED Requirements

### Requirement: DNF source
The system SHALL manage RPM packages via DNF: listing all packages when asked for everything and otherwise only user-installed ones, checking upgrades without changing anything (using cached metadata in dry-run and refreshed metadata otherwise), and applying upgrades with sudo, passing `-y` when auto-yes is set.

**Source:** common/src/sources/system.rs:43

#### Scenario: Full RPM listing includes dependencies
- **GIVEN** a request to list all RPM packages
- **WHEN** the DNF source lists
- **THEN** dependency packages are included

#### Scenario: Only-installed RPM listing
- **GIVEN** a request that excludes dependencies
- **WHEN** the DNF source lists
- **THEN** only user-installed packages are listed

#### Scenario: Upgrade passes auto-yes
- **GIVEN** auto-yes mode
- **WHEN** a DNF upgrade runs
- **THEN** `-y` is passed to the upgrade command

## ADDED Requirements

### Requirement: DNF availability follows the host
The DNF source SHALL be available only on a non-immutable RPM-family host that has `dnf` and `rpm`.

#### Scenario: DNF installed on another distribution
- **GIVEN** a Debian host that has `dnf` and `rpm` installed
- **WHEN** availability is tested
- **THEN** DNF is unavailable

#### Scenario: Fedora
- **GIVEN** a Fedora Workstation host
- **WHEN** availability is tested
- **THEN** DNF is available
