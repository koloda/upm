# Spec Delta

## Purpose

UPM manages popular package systems that work across distributions, namely Snap and Nix user profiles, wherever they are installed.

## ADDED Requirements

### Requirement: Snap source
The Snap source SHALL be available wherever the `snap` tool and a working snap daemon are present, list installed snaps while hiding base and runtime snaps unless everything is requested, check for refreshes without applying them, and upgrade or remove selected snaps through `snap` with sudo.

#### Scenario: Base snaps hidden by default
- **GIVEN** installed application and base snaps
- **WHEN** Snap lists without requesting everything
- **THEN** only the application snaps are listed

#### Scenario: Daemon not working
- **GIVEN** the `snap` tool is installed but the daemon cannot be used
- **WHEN** availability is tested
- **THEN** the source is unavailable

#### Scenario: Targeted refresh
- **GIVEN** one selected snap
- **WHEN** Snap upgrades
- **THEN** only that snap is refreshed

### Requirement: Nix user profile source
The Nix source SHALL manage the packages of the user's own Nix profile in whichever profile format that profile uses, upgrading and removing them without sudo. It MUST NOT mix commands of the two profile formats and MUST NOT list or change system packages declared in NixOS configuration.

#### Scenario: Modern profile
- **GIVEN** a user profile created with `nix profile`
- **WHEN** a package is removed
- **THEN** a `nix profile` command is used

#### Scenario: Legacy profile
- **GIVEN** a user profile created with `nix-env`
- **WHEN** a package is removed
- **THEN** a `nix-env` command is used

#### Scenario: NixOS system packages
- **GIVEN** a NixOS host with packages declared in system configuration
- **WHEN** Nix lists
- **THEN** only user-profile packages are listed

### Requirement: Nix update check
The Nix source SHALL report available updates when the profile format can preview them without changing anything, and otherwise MUST report its packages as unchecked.

#### Scenario: Preview not supported
- **GIVEN** a profile format that cannot preview upgrades
- **WHEN** Nix checks for updates
- **THEN** its packages are reported as unchecked rather than up to date

#### Scenario: Preview changes nothing
- **GIVEN** a profile format that can preview upgrades
- **WHEN** Nix checks for updates
- **THEN** newer versions are reported and the profile is unchanged
