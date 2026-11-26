## Purpose

Some software updates itself rather than through a package manager: standalone programs with their own updater, SDKMAN candidates, the Android SDK, fish plugins and gh extensions, plus applications that update only by hand. UPM tracks them through an "application sources" family. This capability describes their externally observable behaviour.

## Requirements

### Requirement: Self-updating programs
The system SHALL track a known set of separately installed programs, listing only those that are present and not RPM-managed, reading their version, checking for updates through the program's own check command, and updating through the program's own updater.

**Source:** common/src/sources/apps.rs:151

#### Scenario: Only present, non-RPM programs are listed
- **GIVEN** a known program that is absent, and one that is present but RPM-managed
- **WHEN** the tools source lists
- **THEN** neither is listed

#### Scenario: Update availability is parsed
- **GIVEN** a tool whose check output says an update is available
- **WHEN** the tools source checks for updates
- **THEN** the newer version is reported

#### Scenario: Program without a check only updates on upgrade
- **GIVEN** a tracked program that cannot check for updates without updating
- **WHEN** updates are checked
- **THEN** it is reported as unchecked rather than as up to date

### Requirement: SDKMAN candidates
The system SHALL manage SDKMAN candidates, reporting the installed versions of each candidate and detecting newer versions through the SDKMAN candidates API, and MUST NOT run the interactive `sdk upgrade` without a terminal.

**Source:** common/src/sources/apps.rs:270

#### Scenario: Candidate update is detected via the API
- **GIVEN** a candidate whose default version is not installed
- **WHEN** the sdkman source checks for updates
- **THEN** the newer default version is reported

#### Scenario: Interactive upgrade is skipped without a terminal
- **GIVEN** a no-terminal environment
- **WHEN** a full sdkman upgrade runs
- **THEN** the interactive upgrade step is skipped and noted

### Requirement: Android SDK
The system SHALL manage Android SDK packages through sdkmanager, parsing both the old and new listing formats, and update all packages at once or chosen ones individually (translating `/` in a package id to `;`).

**Source:** common/src/sources/apps.rs:429

#### Scenario: Installed packages are listed
- **GIVEN** a device with installed SDK packages
- **WHEN** the android source lists
- **THEN** each installed package appears with its version

#### Scenario: Nested package id is translated
- **GIVEN** a chosen package id containing `/`
- **WHEN** the android source upgrades it
- **THEN** the id is passed to sdkmanager with `;`

### Requirement: Shell plugins and extensions
The system SHALL manage fish plugins (fisher) and gh extensions, listing them with kind-prefixed keys, and update a chosen plugin or extension by name after validating the name contains only safe characters.

**Source:** common/src/sources/apps.rs:486

#### Scenario: Plugin with a strange name is skipped
- **GIVEN** a plugin whose name contains characters outside the safe set
- **WHEN** it is upgraded
- **THEN** it is skipped and a note is recorded

#### Scenario: Named upgrade targets one plugin
- **GIVEN** a specific fish plugin is chosen
- **WHEN** the shell source upgrades
- **THEN** only that plugin is updated

### Requirement: Manual applications
The system SHALL list applications under the user's `Apps` directory (excluding script-only folders and UPM's own directory), Zed, and JetBrains Toolbox applications as items that update by hand, and MUST NOT auto-update them.

**Source:** common/src/sources/apps.rs:612

#### Scenario: Script-only folder is not an application
- **GIVEN** a folder under `~/Apps` containing only shell scripts
- **WHEN** the manual source lists
- **THEN** the folder is not listed

#### Scenario: Manual source cannot be upgraded automatically
- **GIVEN** a full upgrade
- **WHEN** the manual source would be upgraded
- **THEN** it is skipped; only an explicitly chosen manual item is acknowledged as updated by hand

<!-- __inferspec_meta__: {"hash": "74b0d77fe7f61cecff941ccd4b7d049952cbb669254cf91bdb679bb76455dd5e", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
