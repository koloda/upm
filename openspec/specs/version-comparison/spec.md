## Purpose

UPM must compare versions produced by many ecosystems — semantic versions, Go pseudo-versions and vendor suffixes such as `21.0.8-tem`. This capability provides a single comparison that decides whether an available version is newer than the installed one, and that decides update order for the whole catalog.

## Requirements

### Requirement: Numeric comparison
The system SHALL compare version parts numerically rather than lexically, so that `1.10.0` is newer than `1.9.0`.

**Source:** common/src/version.rs:33

#### Scenario: Ten beats nine
- **GIVEN** current `1.9.0`
- **WHEN** it is compared with latest `1.10.0`
- **THEN** the latest is newer

### Requirement: Longer release is newer
The system SHALL treat a release with more components as newer when the shared components are equal, so `1.2.1` is newer than `1.2`.

**Source:** common/src/version.rs:33

#### Scenario: Patch release is newer
- **GIVEN** current `1.2`
- **WHEN** it is compared with latest `1.2.1`
- **THEN** the latest is newer

### Requirement: Prerelease ordering
The system SHALL place a final release above its prereleases, so `1.0.0` is newer than `1.0.0-beta.2`, while among prereleases a higher number is newer.

**Source:** common/src/version.rs:10

#### Scenario: Final beats prerelease
- **GIVEN** current `1.0.0-beta.2`
- **WHEN** it is compared with latest `1.0.0`
- **THEN** the latest is newer

#### Scenario: Higher prerelease number is newer
- **GIVEN** current `1.0.0-beta.2`
- **WHEN** it is compared with latest `1.0.0-beta.10`
- **THEN** the latest is newer

### Requirement: Ignored decorations
The system SHALL ignore a leading `v` and build metadata after `+` when comparing, so `v1.2.3` equals `1.2.3` and `1.2.3+build1` equals `1.2.3+build2`.

**Source:** common/src/version.rs:23

#### Scenario: Leading v and build metadata ignored
- **GIVEN** current `v1.2.3`
- **WHEN** it is compared with latest `1.2.3`
- **THEN** neither is newer

### Requirement: Go pseudo-versions
The system SHALL order Go pseudo-versions by their embedded timestamp, so a later timestamp is newer.

**Source:** common/src/version.rs:10

#### Scenario: Later pseudo-version is newer
- **GIVEN** current `v0.0.0-20260622190343-b1d6741291d4`
- **WHEN** it is compared with latest `v0.0.0-20261118162050-0eef789c9f9a`
- **THEN** the latest is newer

### Requirement: Update decision
The system SHALL decide that an available version is an update exactly when it is strictly newer than the installed version; equal versions MUST NOT be reported as updates.

**Source:** common/src/version.rs:6

#### Scenario: Equal versions are not updates
- **GIVEN** current and latest are the same version
- **WHEN** the update decision is made
- **THEN** no update is reported

<!-- __inferspec_meta__: {"hash": "5d3be0b5d561ee8020141a4f41afc05680d4e8af505e09bf113a1ead3ea41b62", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
