## Purpose

UPM's core operations are `list`, `update` and `upgrade`. The engine performs them over whichever sources are in scope and returns data rather than printing: the terminal and the graphical interface decide how to show the result. This capability covers named-package resolution, streaming listings, update checks and the upgrade run itself.

## Requirements

### Requirement: Named package resolution
The system SHALL turn named packages into per-source keys, matching exact names first and aliases second; if any named package is not found, it MUST report all missing names and MUST NOT perform any changes.

**Source:** common/src/engine.rs:30

#### Scenario: Exact and alias matches resolve
- **GIVEN** a request naming `alpha`, where a package `@scope/alpha` carries the alias `alpha`
- **WHEN** named packages are resolved
- **THEN** the key `@scope/alpha` is picked and reported as found

#### Scenario: One unknown name voids the whole request
- **GIVEN** a request naming a real package and a non-existent one
- **WHEN** named packages are resolved
- **THEN** the resolution fails listing the unknown name, and nothing is changed

### Requirement: Streaming listing
The system SHALL list installed items source by source, handing each finished source to the caller as it becomes ready, and SHALL search by a name fragment over both names and aliases.

**Source:** common/src/engine.rs:113

#### Scenario: Fragment search matches part of a name
- **GIVEN** installed items `firefox` and `glab`
- **WHEN** listing is filtered by the fragment `fire`
- **THEN** `firefox` is listed and `glab` is not

#### Scenario: Naming a package also searches dependencies
- **GIVEN** a package that is installed only as a dependency
- **WHEN** it is listed by name
- **THEN** it is found even though it was not explicitly installed

#### Scenario: Explicitly named empty source is still reported
- **GIVEN** an explicitly named source with nothing installed
- **WHEN** it is listed
- **THEN** an empty listing for that source is emitted

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

### Requirement: Upgrade run and summary
The system SHALL apply the selected updates and return a summary of sources that succeeded, were skipped, failed, or were left unprocessed because of interruption; for a full run it MUST obtain the sudo password once up front when any participating source needs sudo.

**Source:** common/src/engine.rs:271, common/src/engine.rs:303

#### Scenario: Full run keeps sudo alive once
- **GIVEN** a full upgrade of all sources with at least one sudo-needing source in scope
- **WHEN** the run starts
- **THEN** sudo is validated once and kept alive for the rest of the run

#### Scenario: Interruption is recorded
- **GIVEN** an upgrade in progress
- **WHEN** the user interrupts it
- **THEN** remaining sources are not started and the summary reports the interruption

### Requirement: Manual update report
The system SHALL report software that is not updated automatically: RPM packages present in no enabled repository, and applications that update themselves.

**Source:** common/src/engine.rs:345, common/src/engine.rs:351

#### Scenario: Orphan RPMs are listed
- **GIVEN** an installed RPM present in no enabled repository
- **WHEN** the manual report is produced
- **THEN** its name and version appear in the report

<!-- __inferspec_meta__: {"hash": "ac54834ed12a732ef8c53f1f905ecacb1e75d9e8b10880bfcb354c9f10c6b3eb", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
