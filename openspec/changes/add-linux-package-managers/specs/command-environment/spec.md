# Spec Delta

## MODIFIED Requirements

### Requirement: Launcher-independent environment
The context MUST build its own `PATH` that includes the user's usual tool locations (for example `~/.local/bin`, Linuxbrew, `~/go/bin`, the Composer vendor bin, the OpenCode and Mimocode bins, `~/.cargo/bin`, `~/.bun/bin`, and the user and system Nix profile bins) so that results do not depend on whether UPM was started from fish, bash, cron or a desktop shortcut.

**Source:** common/src/context.rs:108

#### Scenario: Tools are found regardless of launcher
- **GIVEN** a tool installed in `~/.cargo/bin` that is absent from the launching shell's `PATH`
- **WHEN** the context checks for that tool
- **THEN** it is found

#### Scenario: Commands run from the home directory
- **GIVEN** any external command
- **WHEN** it is executed
- **THEN** its working directory is the user's home

#### Scenario: Nix from a desktop shortcut
- **GIVEN** Nix installed in the user's profile and not on the launcher's `PATH`
- **WHEN** the context checks for `nix`
- **THEN** it is found

### Requirement: File and package helpers
The system SHALL provide helpers to test whether a file is executable, list a directory's visible entries alphabetically, take a file name, and test whether a file belongs to a package of the host's distribution package manager.

**Source:** common/src/context.rs:355, common/src/context.rs:414, common/src/context.rs:420, common/src/context.rs:433

#### Scenario: RPM-owned file is recognised
- **GIVEN** an RPM-family host and a file owned by an RPM package
- **WHEN** the ownership test runs
- **THEN** the file is reported as distribution-owned

#### Scenario: Debian-owned file is recognised
- **GIVEN** a Debian-family host and a file owned by a Debian package
- **WHEN** the ownership test runs
- **THEN** the file is reported as distribution-owned

#### Scenario: Unowned file
- **GIVEN** a file that no distribution package owns
- **WHEN** the ownership test runs
- **THEN** it is reported as not owned

## ADDED Requirements

### Requirement: Password helpers across distributions
When no helper is configured explicitly, the system SHALL look for a graphical password helper in the locations used by the supported distribution families.

#### Scenario: Debian helper location
- **GIVEN** a graphical password helper installed where Debian places it and no `SUDO_ASKPASS`
- **WHEN** the window needs a sudo password
- **THEN** that helper is used

#### Scenario: Explicit helper wins
- **GIVEN** `SUDO_ASKPASS` pointing to an executable
- **WHEN** a helper is chosen
- **THEN** the configured one is used

### Requirement: Manager-specific success codes
The system SHALL let a source declare non-zero exit codes that mean success, and such a result MUST NOT be recorded as a failed command.

#### Scenario: Declared informational code
- **GIVEN** a command whose declared success codes include 102
- **WHEN** it exits with 102
- **THEN** it counts as successful and is not among failed commands

#### Scenario: Undeclared code still fails
- **GIVEN** the same command
- **WHEN** it exits with 1
- **THEN** it is recorded as failed
