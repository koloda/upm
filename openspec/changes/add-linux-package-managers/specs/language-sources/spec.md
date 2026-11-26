# Spec Delta

## MODIFIED Requirements

### Requirement: Node.js global packages
The system SHALL enumerate globally installed npm packages across the relevant prefixes, skipping local links and files owned by the distribution's package manager, comparing each against the registry's latest version and installing the latest into the prefix where the package was found; it MAY also update pnpm and bun global packages on a full run.

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

#### Scenario: Distribution-owned package is skipped
- **GIVEN** a Debian host with a global npm package installed by a Debian package
- **WHEN** the npm source lists
- **THEN** it is not listed
