# Spec Delta

## Purpose

UPM manages the native packages of popular Linux distributions beyond DNF through APT, Pacman, AUR, Zypper, APK, YUM and a generic RPM source, each following the safety rules of its package manager.

## ADDED Requirements

### Requirement: Native source availability
Each native source SHALL be available only when the host belongs to its family and its required tools are installed.

#### Scenario: APT on Ubuntu
- **GIVEN** an Ubuntu host with `apt-get` and `dpkg-query`
- **WHEN** availability is tested
- **THEN** the APT source is available

#### Scenario: Wrong family
- **GIVEN** an Arch host
- **WHEN** availability of APT, Zypper, APK, YUM and RPM is tested
- **THEN** all of them are unavailable

### Requirement: Native inventory
A native source SHALL list the packages the user asked for by default and every installed package when everything is requested. When the manager cannot tell which packages the user asked for, it SHALL list every package and say so.

#### Scenario: Dependencies hidden by default
- **GIVEN** an APT package installed automatically as a dependency
- **WHEN** APT lists without requesting everything
- **THEN** that package is not listed

#### Scenario: Everything includes dependencies
- **GIVEN** a request for all packages
- **WHEN** a native source lists
- **THEN** dependency packages are included

#### Scenario: Origin of installation unknown
- **GIVEN** a manager that cannot report user-requested packages
- **WHEN** the source lists without requesting everything
- **THEN** all packages are listed with a diagnostic explaining why

### Requirement: Native update check
A native source SHALL report installed and available versions without changing installed packages. In dry-run it MUST run no command requiring sudo. Otherwise it SHALL use fresh metadata, refreshing it with sudo where the manager requires that, unless a source-specific requirement below says otherwise. A manager's "no updates" result MUST NOT be treated as a failure.

#### Scenario: Dry-run check needs no sudo
- **GIVEN** dry-run mode
- **WHEN** a native source checks for updates
- **THEN** no command requiring sudo is run

#### Scenario: Up-to-date system
- **GIVEN** the manager reports there are no updates
- **WHEN** the check finishes
- **THEN** the result is empty and not failed

#### Scenario: Metadata refresh fails
- **GIVEN** a metadata refresh that fails
- **WHEN** the check finishes
- **THEN** the source reports the check as failed

### Requirement: Native changes respect manager safeguards
Except where a source-specific requirement forbids a change, a native source SHALL upgrade and remove through its own manager with sudo, touching only the selected packages when a selection is given. It MUST skip confirmation only when auto-yes is set and MUST NOT pass options that override the manager's protections against breaking the system.

#### Scenario: Targeted removal
- **GIVEN** one selected package
- **WHEN** a native source removes it
- **THEN** the manager's remove command names only that package

#### Scenario: Confirmation is kept without auto-yes
- **GIVEN** auto-yes is not set
- **WHEN** an upgrade or removal command is built
- **THEN** it carries no confirmation-skipping option

#### Scenario: Protected package
- **GIVEN** a removal the manager refuses because the package is essential or required
- **WHEN** the removal runs
- **THEN** the refusal is reported as a failure and UPM does not retry with a forcing option

#### Scenario: Empty selection removes nothing
- **GIVEN** no selected packages
- **WHEN** removal is requested
- **THEN** no command is run

### Requirement: APT source
The APT source SHALL manage Debian packages, including foreign-architecture ones as distinct items. A full upgrade MUST NOT remove installed packages, and a targeted upgrade MUST NOT install packages that were not already installed except as required dependencies.

#### Scenario: Full upgrade never removes
- **GIVEN** a full APT upgrade
- **WHEN** an update would require removing a package
- **THEN** that update is held back instead

#### Scenario: Multi-architecture package
- **GIVEN** the same library installed for two architectures
- **WHEN** APT lists everything
- **THEN** the two appear as separate items that can be selected independently

### Requirement: APT without a terminal
When there is no terminal and auto-yes is set, the APT source MUST NOT wait for configuration questions and MUST keep locally modified configuration files.

#### Scenario: Changed configuration file during a window upgrade
- **GIVEN** an upgrade started from the window that ships a new version of a modified configuration file
- **WHEN** the upgrade runs
- **THEN** it completes without prompting and the local file is kept

### Requirement: Pacman source
The Pacman source SHALL manage packages that come from the configured repositories and MUST NOT list foreign packages. Its update check MUST NOT synchronise the system package database; when no safe fresh check is available it SHALL use cached data and say so.

#### Scenario: Safe fresh check
- **GIVEN** `checkupdates` is installed
- **WHEN** Pacman checks for updates outside dry-run
- **THEN** available updates are reported without synchronising the system database

#### Scenario: No safe fresh check
- **GIVEN** `checkupdates` is not installed
- **WHEN** Pacman checks for updates
- **THEN** results come from cached data and a diagnostic says they may be stale

### Requirement: No partial upgrades with Pacman
The Pacman source MUST upgrade only the whole system. A request to upgrade selected Pacman packages MUST run nothing, be reported as skipped and explain that the whole source has to be upgraded.

#### Scenario: Whole source upgrade
- **GIVEN** a full Pacman upgrade
- **WHEN** it runs
- **THEN** the database is synchronised and all packages are upgraded in one command

#### Scenario: Selected package upgrade
- **GIVEN** a request to upgrade one Pacman package
- **WHEN** it runs
- **THEN** no command is run, the source is skipped and a note explains why

### Requirement: AUR source
The AUR source SHALL manage foreign packages on Arch-family hosts, listing them and removing selected ones through Pacman. With `paru` or `yay` installed it SHALL check for and apply AUR updates, whole or selected; without a helper it MUST report their update state as unknown and skip upgrades with an explanation.

#### Scenario: Update through a helper
- **GIVEN** a helper is installed and a foreign package has a newer AUR release
- **WHEN** AUR checks for updates
- **THEN** that update is reported

#### Scenario: No helper
- **GIVEN** neither helper is installed
- **WHEN** AUR checks for updates
- **THEN** the packages are reported as unchecked rather than up to date

#### Scenario: Foreign package unknown to the AUR
- **GIVEN** a locally built package that the AUR does not know
- **WHEN** AUR lists
- **THEN** it is listed and can be removed

### Requirement: AUR builds need a terminal
AUR packages are built from user-submitted scripts, so the system MUST NOT upgrade them without a terminal in which they can be reviewed. It MUST skip the upgrade with a note instead.

#### Scenario: Upgrade from the window
- **GIVEN** an AUR upgrade requested from the graphical interface
- **WHEN** it runs
- **THEN** nothing is built and a note says to upgrade AUR packages from a terminal

#### Scenario: Interactive review in a terminal
- **GIVEN** a terminal and no auto-yes
- **WHEN** an AUR upgrade runs
- **THEN** the helper runs interactively with its review prompts

### Requirement: Zypper source
The Zypper source SHALL manage openSUSE and SLE packages, use a distribution upgrade for a full upgrade on rolling releases and a package update elsewhere, and treat "reboot required" and "restart required" results as success with a note.

#### Scenario: Tumbleweed full upgrade
- **GIVEN** an openSUSE Tumbleweed host
- **WHEN** a full upgrade runs
- **THEN** a distribution upgrade is performed

#### Scenario: Leap full upgrade
- **GIVEN** an openSUSE Leap host
- **WHEN** a full upgrade runs
- **THEN** a package update is performed

#### Scenario: Reboot required
- **GIVEN** an upgrade that succeeds and asks for a reboot
- **WHEN** it finishes
- **THEN** the source is successful and a note mentions the reboot

### Requirement: APK source
The APK source SHALL manage Alpine packages, treating the packages named in the system's world set as the ones the user asked for.

#### Scenario: World packages by default
- **GIVEN** a package present only as a dependency
- **WHEN** APK lists without requesting everything
- **THEN** it is not listed

#### Scenario: Targeted upgrade
- **GIVEN** one selected package with an update
- **WHEN** APK upgrades it
- **THEN** only that package and its required dependencies are upgraded

### Requirement: YUM source
The YUM source SHALL manage RPM packages on RPM-family hosts that have `yum` and no `dnf`.

#### Scenario: Legacy system
- **GIVEN** an RPM host with `yum` only
- **WHEN** sources are offered
- **THEN** YUM is offered

#### Scenario: YUM is an alias of DNF
- **GIVEN** an RPM host where both `yum` and `dnf` exist
- **WHEN** sources are offered
- **THEN** only DNF is offered and packages are not listed twice

### Requirement: Generic RPM source
On an RPM or openSUSE host with no manager UPM can safely drive, the system SHALL offer a read-only RPM source that lists all installed RPM packages, reports their update state as unknown, skips upgrades with an explanation and offers no removal.

#### Scenario: RPM system without a supported manager
- **GIVEN** an RPM host with `rpm` but none of `dnf`, `yum` or `zypper`
- **WHEN** sources are offered
- **THEN** the RPM source is offered

#### Scenario: Read-only behaviour
- **GIVEN** the RPM source
- **WHEN** an upgrade or removal is requested
- **THEN** no command changes the system
