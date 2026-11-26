## Purpose

The catalog is UPM's screen model: it merges the installed inventory with the results of update checks into a single set of rows, filters, per-source counters and an upgrade plan. It contains no terminal or Qt code, so any interface can render it. This capability defines package states, filtering, summaries and how a user's selection becomes an upgrade plan.

## Requirements

### Requirement: Package state
The system SHALL classify each package as not-checked, upgradable, up-to-date or unknown; packages of the manual source MUST always be unknown, and a source that is unavailable, failed its check or left a key unchecked MUST yield unknown for the affected rows.

**Source:** common/src/catalog.rs:14, common/src/catalog.rs:123

#### Scenario: Manual packages are always unknown
- **GIVEN** a package belonging to the manual source
- **WHEN** its state is computed
- **THEN** it is unknown regardless of any check

#### Scenario: Failed check makes rows unknown
- **GIVEN** a source whose update check failed
- **WHEN** its rows are computed
- **THEN** every row of that source is unknown

#### Scenario: Nothing is upgradable before a check
- **GIVEN** a catalog that has only loaded inventory
- **WHEN** rows filtered to upgradable are requested
- **THEN** the result is empty

### Requirement: Matching updates to items
The system SHALL combine each installed item with its update by key, and when a key is not unique MUST disambiguate the update by the item's description.

**Source:** common/src/catalog.rs:75

#### Scenario: Same key in two prefixes
- **GIVEN** the same package installed twice with the same key but different descriptions, with an update for one description
- **WHEN** rows are computed
- **THEN** only the matching instance is marked upgradable

### Requirement: Dependency updates are shown
The system SHALL show updates for dependencies that are not in the user's own installed list as upgradable rows.

**Source:** common/src/catalog.rs:104

#### Scenario: Dependency update appears
- **GIVEN** a check that reports an update for a package not present in the inventory
- **WHEN** rows filtered to upgradable are requested
- **THEN** that dependency appears as an upgradable row

### Requirement: Filtering
The system SHALL filter rows by source, by "only upgradable", and by a case-insensitive substring of the package name, combining all three.

**Source:** common/src/catalog.rs:173

#### Scenario: Source and search combine
- **GIVEN** packages across several sources
- **WHEN** rows are requested for one source and a name fragment
- **THEN** only rows of that source whose name contains the fragment are returned

### Requirement: Per-source summaries
The system SHALL report for each source how many packages are installed, how many updates are available, whether it has been checked and whether it failed.

**Source:** common/src/catalog.rs:184

#### Scenario: Summary counts match
- **GIVEN** a checked source with installed packages and updates
- **WHEN** summaries are computed
- **THEN** its installed and update counts equal the corresponding rows

#### Scenario: Reloading a source forgets its check
- **GIVEN** a checked source
- **WHEN** its inventory is reloaded
- **THEN** it is no longer marked as checked

### Requirement: Upgrade plan from selection
The system SHALL build an upgrade plan: when specific packages are selected, only those are upgraded (excluding the manual source); when none are selected, the current source is upgraded, or everything except firmware when no source is current; a stale or unknown selection MUST NOT become a full upgrade, and the manual source MUST NOT be upgradeable.

**Source:** common/src/catalog.rs:199

#### Scenario: Selection upgrades only selected
- **GIVEN** a selection of specific packages
- **WHEN** a plan is built
- **THEN** the plan targets only those packages

#### Scenario: Unknown selection is not a full upgrade
- **GIVEN** a selection whose package no longer exists
- **WHEN** a plan is built
- **THEN** the plan is empty

#### Scenario: Manual source cannot be planned
- **GIVEN** the manual source is current, or a manual package is selected
- **WHEN** a plan is built
- **THEN** the plan is empty

#### Scenario: All view skips firmware
- **GIVEN** no current source and no selection
- **WHEN** a plan is built
- **THEN** it covers everything except firmware

<!-- __inferspec_meta__: {"hash": "bceb30297cbc1000194ab701d8fb72173c22e53e1cf9c9c3a34035abcbe80708", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
