## Purpose

UPM (universal manager of everything installed — "універсальний менеджер всього, що встановлено") treats every kind of installed software as a *source* behind one common contract. This capability defines that contract — the `Source` trait — together with the fixed registry that lists all sources and the canonical ids and synonyms used to name them. Adding a new kind of managed software is deliberately cheap: implement the trait and register the source.

## Requirements

### Requirement: Common source contract
The system SHALL expose every managed kind of software as a source implementing a single trait that provides its static id, availability, inventory, outdated check, upgrade and sudo hint.

**Source:** common/src/source.rs:155

#### Scenario: Source answers every operation
- **GIVEN** a source implementation
- **WHEN** the core asks it for id, availability, inventory, outdated, upgrade and needs_sudo
- **THEN** it returns each value without any source-specific code in the caller

### Requirement: Stable source identifiers
Each source MUST report a stable static id, and the registry MUST list exactly those ids in the same order.

**Source:** common/src/source.rs:156, common/src/sources/mod.rs:29

#### Scenario: Registry ids match source ids
- **GIVEN** the registry
- **WHEN** each registered source reports its id
- **THEN** the ids equal the registry list dnf, flatpak, brew, appimage, npm, composer, go, rust, python, tools, sdkman, android, shell, firmware, manual

### Requirement: Fixed source order
The registry MUST return sources in a fixed canonical order used both for execution and for output.

**Source:** common/src/sources/mod.rs:9

#### Scenario: Sources are enumerated in canonical order
- **GIVEN** the registry
- **WHEN** the core enumerates sources for a full run
- **THEN** they are visited in the order DNF, Flatpak, Brew, AppImage, Npm, Composer, Go, Rust, Python, Tools, Sdkman, Android, Shell, Firmware, Manual

### Requirement: Canonical names and synonyms
The system SHALL resolve a user-typed source word to a canonical id, case-insensitively, accepting the synonyms node/nodejs→npm, py/pip→python, cargo→rust, rpm→dnf, golang→go, homebrew→brew, appimages→appimage and sdk→sdkman.

**Source:** common/src/sources/mod.rs:35

#### Scenario: A synonym resolves to its canonical id
- **GIVEN** the word `cargo`
- **WHEN** it is resolved as a source name
- **THEN** the canonical id `rust` is returned

#### Scenario: An unknown word is not a source
- **GIVEN** the word `firefox`
- **WHEN** it is resolved as a source name
- **THEN** no source id is returned

### Requirement: Availability gating
A source MUST determine for itself whether it is present on this computer, and the core MUST NOT query an unavailable source.

**Source:** common/src/source.rs:159

#### Scenario: Absent source is skipped
- **GIVEN** a source whose tool is not installed
- **WHEN** the core considers running it
- **THEN** the source reports itself unavailable and is not queried

### Requirement: Request result shape
The system SHALL return installed items and updates as rows accompanied by a failure flag and a list of keys whose state could not be determined.

**Source:** common/src/source.rs:80

#### Scenario: Partial failure is reported alongside valid rows
- **GIVEN** a source query where the listing failed
- **WHEN** the query returns
- **THEN** it carries a failed flag while still returning any valid rows and any unchecked keys

### Requirement: Cheap extensibility
Adding a new kind of managed software MUST require only implementing the source trait and adding it to the registry.

**Source:** common/src/source.rs:153, common/src/sources/mod.rs:9

#### Scenario: New source needs no core changes
- **GIVEN** a new source implementation
- **WHEN** it is added to the registry
- **THEN** it participates in list/update/upgrade through the existing core operations

<!-- __inferspec_meta__: {"hash": "715f8afa5fd4f7485b90b90b5e2ac824074b83d427a00ed74c50e7b564dfd051", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
