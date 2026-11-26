# Spec Delta

## Purpose

UPM identifies which kind of operating system it runs on, so that only the distribution package sources that belong to that host are offered to the user.

## ADDED Requirements

### Requirement: Distribution family detection
The system SHALL determine the host's distribution family (RPM, openSUSE, Debian, Arch or Alpine) from the operating system's release identity, preferring the distribution's own id and then the ids it declares itself similar to, in their declared order.

#### Scenario: Known distribution
- **GIVEN** a host identifying itself as Fedora
- **WHEN** the family is determined
- **THEN** it is the RPM family

#### Scenario: Derivative is recognised through its declared similarity
- **GIVEN** a host with an unfamiliar id that declares itself similar to `ubuntu debian`
- **WHEN** the family is determined
- **THEN** it is the Debian family

#### Scenario: openSUSE is separate from the RPM family
- **GIVEN** a host identifying itself as openSUSE Tumbleweed
- **WHEN** the family is determined
- **THEN** it is the openSUSE family

### Requirement: At most one native family
The system MUST activate the native sources of at most one distribution family. The mere presence of another family's package tools MUST NOT activate that family.

#### Scenario: Foreign tools on Fedora
- **GIVEN** a Fedora host that also has `apt` and `dpkg` installed
- **WHEN** sources are offered
- **THEN** DNF is offered and APT is not

#### Scenario: Native tool is missing
- **GIVEN** a Debian-family host without `apt-get`
- **WHEN** sources are offered
- **THEN** no native source of another family is offered in its place

### Requirement: Unidentified distribution fallback
When the release identity matches no known family and is not NixOS or macOS, the system SHALL choose at most one family whose package database and tools are both present, trying Debian, Arch, Alpine, openSUSE and RPM in that order.

#### Scenario: Unknown distribution with a pacman database
- **GIVEN** an unrecognised distribution with a pacman database and `pacman`
- **WHEN** the family is determined
- **THEN** it is the Arch family

#### Scenario: Nothing recognisable
- **GIVEN** an unrecognised host with no known package database
- **WHEN** sources are offered
- **THEN** no native source is offered and other sources keep working

### Requirement: Hosts without a native family
On NixOS and macOS the system MUST NOT offer any native distribution source, while distribution-independent sources remain available according to what is installed.

#### Scenario: NixOS
- **GIVEN** a NixOS host
- **WHEN** sources are offered
- **THEN** no native distribution source is offered and Nix is offered

#### Scenario: macOS
- **GIVEN** a macOS host with Homebrew
- **WHEN** sources are offered
- **THEN** Homebrew is offered and no Linux native source is offered

### Requirement: Immutable hosts are read-only
On a host whose system packages are managed atomically or mounted read-only (ostree-based systems, openSUSE MicroOS, Aeon and Leap Micro, SteamOS), the system MUST NOT modify native packages. RPM and openSUSE hosts SHALL offer the generic RPM source; Arch hosts SHALL offer Pacman for listing only.

#### Scenario: Fedora Atomic desktop
- **GIVEN** an ostree-booted RPM host that has `dnf` installed
- **WHEN** sources are offered
- **THEN** the generic RPM source is offered and DNF is not

#### Scenario: SteamOS
- **GIVEN** a SteamOS host
- **WHEN** sources are offered
- **THEN** Pacman lists packages, declines upgrade and removal, and AUR is not offered
