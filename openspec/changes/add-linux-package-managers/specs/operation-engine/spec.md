# Spec Delta

## MODIFIED Requirements

### Requirement: Update check without changes
The system SHALL check for available updates without changing anything installed, reporting per source its availability, updates, failures, undetermined keys and named packages that have no update, and MUST skip the manual source unless it is explicitly named.

**Source:** common/src/engine.rs:220

#### Scenario: Check is read-only
- **GIVEN** sources with available updates
- **WHEN** an update check runs
- **THEN** results are reported per source and nothing is installed

#### Scenario: Manual source is not checked by default
- **GIVEN** a full update check
- **WHEN** the manual source is in scope
- **THEN** it is skipped because it cannot be checked automatically

#### Scenario: Named package without an update is reported
- **GIVEN** an explicitly named, already-current package
- **WHEN** the update check finishes
- **THEN** that package is reported as having no update found

#### Scenario: Absent source is silent
- **GIVEN** a full update check on a Debian host
- **WHEN** it reaches the DNF source
- **THEN** nothing is reported for DNF

#### Scenario: Named absent source is reported
- **GIVEN** a Debian host
- **WHEN** an update check names `dnf`
- **THEN** DNF is reported as unavailable

### Requirement: Upgrade run and summary
The system SHALL apply the selected updates and return a summary of sources that succeeded, were skipped, failed, or were left unprocessed because of interruption; for a full run it MUST obtain the sudo password once up front when any participating source needs sudo. A source absent from this computer does not participate and MUST NOT appear in the summary unless it was named.

**Source:** common/src/engine.rs:271, common/src/engine.rs:303

#### Scenario: Full run keeps sudo alive once
- **GIVEN** a full upgrade of all sources with at least one sudo-needing source in scope
- **WHEN** the run starts
- **THEN** sudo is validated once and kept alive for the rest of the run

#### Scenario: Interruption is recorded
- **GIVEN** an upgrade in progress
- **WHEN** the user interrupts it
- **THEN** remaining sources are not started and the summary reports the interruption

#### Scenario: Absent sources do not request sudo
- **GIVEN** a host where no present source needs sudo
- **WHEN** a full upgrade starts
- **THEN** no sudo password is requested

#### Scenario: Absent sources are not listed as skipped
- **GIVEN** a full upgrade on an Arch host
- **WHEN** the summary is produced
- **THEN** it does not mention DNF or APT

### Requirement: Manual update report
The system SHALL report software that is not updated automatically: applications that update themselves and, on hosts where DNF is the active native source, RPM packages present in no enabled repository.

**Source:** common/src/engine.rs:345, common/src/engine.rs:351

#### Scenario: Orphan RPMs are listed
- **GIVEN** an installed RPM present in no enabled repository on a DNF host
- **WHEN** the manual report is produced
- **THEN** its name and version appear in the report

#### Scenario: No RPM query on other hosts
- **GIVEN** a Debian host that has `dnf` installed
- **WHEN** the manual report is produced
- **THEN** DNF is not run and no RPM section appears
