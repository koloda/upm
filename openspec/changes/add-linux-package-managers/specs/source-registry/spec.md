# Spec Delta

## MODIFIED Requirements

### Requirement: Stable source identifiers
Each source MUST report a stable static id, and the registry MUST list exactly those ids in the same order.

**Source:** common/src/source.rs:156, common/src/sources/mod.rs:29

#### Scenario: Registry ids match source ids
- **GIVEN** the registry
- **WHEN** each registered source reports its id
- **THEN** the ids equal the registry list dnf, yum, zypper, rpm, apt, pacman, aur, apk, flatpak, snap, nix, brew, appimage, npm, composer, go, rust, python, tools, sdkman, android, shell, firmware, manual

### Requirement: Fixed source order
The registry MUST return sources in a fixed canonical order used both for execution and for output.

**Source:** common/src/sources/mod.rs:9

#### Scenario: Sources are enumerated in canonical order
- **GIVEN** the registry
- **WHEN** the core enumerates sources for a full run
- **THEN** they are visited in the order DNF, YUM, Zypper, RPM, APT, Pacman, AUR, APK, Flatpak, Snap, Nix, Brew, AppImage, Npm, Composer, Go, Rust, Python, Tools, Sdkman, Android, Shell, Firmware, Manual

### Requirement: Canonical names and synonyms
The system SHALL resolve a user-typed source word to a canonical id, case-insensitively, accepting the synonyms node/nodejs→npm, py/pip→python, cargo→rust, golang→go, homebrew→brew, appimages→appimage, sdk→sdkman and deb/dpkg→apt. The word `rpm` SHALL resolve to the host's active RPM-based source (DNF, YUM, Zypper or the generic RPM source).

**Source:** common/src/sources/mod.rs:35

#### Scenario: A synonym resolves to its canonical id
- **GIVEN** the word `cargo`
- **WHEN** it is resolved as a source name
- **THEN** the canonical id `rust` is returned

#### Scenario: An unknown word is not a source
- **GIVEN** the word `firefox`
- **WHEN** it is resolved as a source name
- **THEN** no source id is returned

#### Scenario: rpm on Fedora
- **GIVEN** a Fedora host
- **WHEN** the word `rpm` is resolved
- **THEN** the canonical id `dnf` is returned

#### Scenario: rpm on openSUSE
- **GIVEN** an openSUSE host
- **WHEN** the word `rpm` is resolved
- **THEN** the canonical id `zypper` is returned

#### Scenario: rpm without an RPM source
- **GIVEN** a Debian host
- **WHEN** the word `rpm` is resolved
- **THEN** the canonical id `rpm` is returned and that source is reported as absent
