## Purpose

UPM manages globally installed language-package-manager software: Node.js global packages (npm), global Composer packages, Go binaries installed with `go install`, Rust toolchains and crates installed with `cargo install`, and Python tools installed with pip, uv and pipx. Each knows how to enumerate its installs, compare them with the latest available and update them. This capability describes their externally observable behaviour.

## Requirements

### Requirement: Node.js global packages
The system SHALL enumerate globally installed npm packages across the relevant prefixes, skipping local links and RPM-managed files, comparing each against the registry's latest version and installing the latest into the prefix where the package was found; it MAY also update pnpm and bun global packages on a full run.

**Source:** common/src/sources/lang.rs:85

#### Scenario: Scoped packages are listed
- **GIVEN** a scoped package such as `@scope/tool` installed globally
- **WHEN** the npm source lists
- **THEN** the scoped package appears with its version

#### Scenario: Local link is skipped
- **GIVEN** a package directory that is a symbolic link (an `npm link`)
- **WHEN** the npm source lists
- **THEN** it is not listed

#### Scenario: Update installs into the discovered prefix
- **GIVEN** an outdated global package located in a non-default prefix
- **WHEN** the npm source upgrades it
- **THEN** the latest version is installed with that prefix

### Requirement: Global Composer packages
The system SHALL enumerate global Composer packages and, on a full run, also show major updates that Composer itself will not apply.

**Source:** common/src/sources/lang.rs:201

#### Scenario: Outdated packages are listed
- **GIVEN** a global Composer package with a newer release
- **WHEN** the Composer source checks for updates
- **THEN** the newer version is reported

#### Scenario: Full run shows blocked majors
- **GIVEN** a full Composer upgrade
- **WHEN** it runs
- **THEN** direct outdated packages are also shown after the update

### Requirement: Go binaries
The system SHALL enumerate executables in the Go binary directory, read their module and version from the binary's build information, treat locally built binaries (unknown version) as uncheckable, and update a package by installing its `@latest`.

**Source:** common/src/sources/lang.rs:334

#### Scenario: Build info yields module and version
- **GIVEN** a binary carrying Go build information
- **WHEN** the Go source inspects it
- **THEN** its package path, module and version are extracted

#### Scenario: Locally built binary is not checked
- **GIVEN** a binary whose version is empty or `(devel)`
- **WHEN** the Go source checks for updates
- **THEN** the binary is reported as unchecked and not updated

### Requirement: Rust toolchains and crates
The system SHALL enumerate rustup toolchains and crates installed with `cargo install`, comparing registry crates with crates.io and treating crates installed from git or a local path as manually updated.

**Source:** common/src/sources/lang.rs:466

#### Scenario: Toolchains are listed
- **GIVEN** rustup with installed toolchains
- **WHEN** the Rust source lists
- **THEN** each toolchain appears as an item

#### Scenario: Git or local crate is not auto-updated
- **GIVEN** a crate installed from git or a local path
- **WHEN** the Rust source checks or upgrades
- **THEN** it is reported as needing manual update instead of being updated

### Requirement: Python tools
The system SHALL enumerate Python tools installed by pip (user site), uv and pipx, distinguishing explicitly requested packages from dependencies, matching names case- and separator-insensitively as PyPI does, and reporting packages left behind by a Python version that no longer exists.

**Source:** common/src/sources/lang.rs:708

#### Scenario: PyPI name normalisation
- **GIVEN** the name `Tree_Sitter.C-Sharp`
- **WHEN** it is normalised for comparison with PyPI
- **THEN** it becomes `tree-sitter-c-sharp`

#### Scenario: pipx cannot be checked only
- **GIVEN** a pipx-installed tool
- **WHEN** the Python source checks for updates
- **THEN** the tool is reported as unchecked because pipx cannot check without updating

#### Scenario: Orphan of a removed Python is reported
- **GIVEN** packages installed under a Python version that is no longer present
- **WHEN** the Python source lists
- **THEN** those packages appear marked as not working

<!-- __inferspec_meta__: {"hash": "a14e9f5534bec3e2efe34aa6376e4be9a7881198553b2c3394c58d9bdef98caf", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
