## Purpose

UPM manages several *system* sources: the distribution's RPM packages (DNF), Flatpak applications, Homebrew formulae and casks, AppImage applications registered with the AppImage manager, and device firmware via fwupd. Each knows how to list its installed items, check for updates and apply updates. This capability describes the externally observable behaviour of those system sources.

## Requirements

### Requirement: DNF source
The system SHALL manage RPM packages via DNF: listing all packages when asked for everything and otherwise only user-installed ones, checking upgrades without changing anything (using cached metadata in dry-run and refreshed metadata otherwise), and applying upgrades with sudo, passing `-y` when auto-yes is set.

**Source:** common/src/sources/system.rs:43

#### Scenario: Full RPM listing includes dependencies
- **GIVEN** a request to list all RPM packages
- **WHEN** the DNF source lists
- **THEN** dependency packages are included

#### Scenario: Only-installed RPM listing
- **GIVEN** a request that excludes dependencies
- **WHEN** the DNF source lists
- **THEN** only user-installed packages are listed

#### Scenario: Upgrade passes auto-yes
- **GIVEN** auto-yes mode
- **WHEN** a DNF upgrade runs
- **THEN** `-y` is passed to the upgrade command

### Requirement: Firmware source
The system SHALL manage firmware via fwupd: list only devices flagged updatable, treat "no updates" as a normal result, and update a chosen device by its id or all devices when none is chosen.

**Source:** common/src/sources/system.rs:393

#### Scenario: Non-updatable devices are hidden
- **GIVEN** a device that is not flagged updatable
- **WHEN** the firmware source lists devices
- **THEN** the device is not listed

#### Scenario: No updates is not an error
- **GIVEN** fwupd reports there are no updates
- **WHEN** the firmware source checks for updates
- **THEN** the result is empty but not a failure

### Requirement: Flatpak source
The system SHALL manage Flatpak applications across user and system installations, distinguish applications by installation and application id, and report a rebuild when the update's version equals the installed one.

**Source:** common/src/sources/system.rs:119

#### Scenario: Rebuild is shown when version is unchanged
- **GIVEN** an application whose update has the same version as installed
- **WHEN** the flatpak source checks for updates
- **THEN** it reports a rebuild with the branch

#### Scenario: Same app in two installations
- **GIVEN** the same application installed both as user and as system
- **WHEN** the flatpak source lists
- **THEN** the two installations appear as separate items

### Requirement: Homebrew source
The system SHALL manage Homebrew formulae and casks without auto-updating at list time, restrict formulae to those installed on request unless everything is requested, and keep old versions on upgrade.

**Source:** common/src/sources/system.rs:239

#### Scenario: Only requested formulae by default
- **GIVEN** a formula installed only as a dependency
- **WHEN** the brew source lists without the all flag
- **THEN** the formula is not listed

#### Scenario: Old versions are kept on upgrade
- **GIVEN** a brew upgrade
- **WHEN** it runs
- **THEN** cleanup and autoremove are disabled

### Requirement: AppImage source
The system SHALL manage AppImage applications registered with the AppImage manager, is available only when the manager and its registry are present, and updates all applications at once or a chosen one individually.

**Source:** common/src/sources/system.rs:343

#### Scenario: Named update targets one application
- **GIVEN** a specific AppImage application is chosen
- **WHEN** the appimage source upgrades
- **THEN** only that application is updated

#### Scenario: Source is unavailable without the manager
- **GIVEN** the AppImage manager is not installed
- **WHEN** availability is tested
- **THEN** the source is unavailable

<!-- __inferspec_meta__: {"hash": "4ff8e2397e94607594b6fbb568d3b70eb2751849d2e9b046467228ef4484de76", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
