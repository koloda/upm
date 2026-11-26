# Spec Delta

## MODIFIED Requirements

### Requirement: Common source contract
The system SHALL expose every managed kind of software as a source implementing a single trait that provides its static id, availability, inventory, outdated check, upgrade, an optional uninstall operation and a sudo hint. A source that cannot uninstall its items MUST decline removal instead of deleting files itself.

**Source:** common/src/source.rs:155

#### Scenario: Source answers every operation
- **GIVEN** a source implementation
- **WHEN** the core asks it for id, availability, inventory, outdated, upgrade, removal and needs_sudo
- **THEN** it returns each value without any source-specific code in the caller

#### Scenario: Source without removal declines
- **GIVEN** a source with no native uninstall command
- **WHEN** the core asks it to remove a package
- **THEN** it reports the removal as skipped and does not delete files

### Requirement: Cheap extensibility
Adding a new kind of managed software MUST require only implementing the source trait and adding it to the registry.

**Source:** common/src/source.rs:153, common/src/sources/mod.rs:9

#### Scenario: New source needs no core changes
- **GIVEN** a new source implementation
- **WHEN** it is added to the registry
- **THEN** it participates in list/update/upgrade through the existing core operations and, when it declares removal, in removal through the same core operation

## ADDED Requirements

### Requirement: Removal feasibility declaration
Each source MUST declare whether it can uninstall its items, and that declaration MUST be available before any removal is attempted.

**Source:** common/src/source.rs:155

#### Scenario: Feasibility is known before removal
- **GIVEN** a set of installed items
- **WHEN** removal is being planned
- **THEN** the system can tell for each item whether its source can uninstall it without running a command
