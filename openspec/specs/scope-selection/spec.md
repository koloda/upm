## Purpose

A command runs against a *scope*: which sources and which named packages the user selected. This capability defines that scope and the decisions derived from it — which sources are allowed, which are actually active for update or upgrade, and how named selections turn into per-source picks. An empty scope means "everything except firmware".

## Requirements

### Requirement: Scope contents
The system SHALL represent a request as a scope holding selected sources, named packages (each optionally qualified with a source), an only-list, a skip-list and a firmware flag.

**Source:** common/src/scope.rs:9, common/src/scope.rs:16

#### Scenario: Empty scope means everything but firmware
- **GIVEN** an empty scope
- **WHEN** it is asked whether firmware is in scope
- **THEN** firmware is excluded

### Requirement: Only, skip and firmware gating
The system SHALL decide a source's participation from the only-list, the skip-list and the firmware flag: a source in the skip-list is excluded, a source not in a non-empty only-list is excluded, and firmware participates only when explicitly requested.

**Source:** common/src/scope.rs:36

#### Scenario: Skipped source is out of scope
- **GIVEN** a scope whose skip-list contains `dnf`
- **WHEN** the scope is asked about `dnf`
- **THEN** it is out of scope

#### Scenario: Firmware needs an explicit request
- **GIVEN** a scope without a firmware request
- **WHEN** the scope is asked about `firmware`
- **THEN** it is out of scope

### Requirement: Active sources for a run
The system SHALL decide whether a source is active for update/upgrade from whether named packages are present and from the per-source picks: with no targets and no picks only allowed sources are active; with specific picks only picked (or, absent bare targets, named) sources are touched.

**Source:** common/src/scope.rs:62

#### Scenario: Picks restrict the run
- **GIVEN** per-source picks for one source and no named targets
- **WHEN** active sources are determined
- **THEN** only the picked source is active

#### Scenario: Untargeted bare names keep the run broad
- **GIVEN** a scope with a package named without a source and no picks
- **WHEN** active sources are determined
- **THEN** the run is not narrowed to a single source

### Requirement: Selection helpers
The system SHALL expose whether a scope names a source, whether it allows a source, whether it contains a bare target and whether it is a full pass.

**Source:** common/src/scope.rs:42, common/src/scope.rs:52, common/src/scope.rs:57

#### Scenario: Everything detection
- **GIVEN** a scope with no selected sources and no named packages
- **WHEN** it is asked whether it is a full pass
- **THEN** it reports yes

### Requirement: Picks to per-source selection
The system SHALL convert named packages into a per-source map of keys, and expose a source's picked keys as a selection that is either "whole source" or a specific key list.

**Source:** common/src/scope.rs:28, common/src/scope.rs:30

#### Scenario: Missing source is a whole-source selection
- **GIVEN** a per-source picks map with no entry for a source
- **WHEN** that source's selection is requested
- **THEN** it is an untargeted selection meaning the whole source

<!-- __inferspec_meta__: {"hash": "b1a0839ccd0b4dfa50b01cda6fe8eb29451fc42b31697536570ffc0ee3097554", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
