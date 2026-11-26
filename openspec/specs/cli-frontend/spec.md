## Purpose

`upm` is the command-line front end over the UPM core. It parses commands and options, refuses to run with superuser privileges, resolves the requested sources and packages, and renders the core's results as text with colour and aligned tables. This capability describes the CLI's observable contract: commands, options, output and exit codes.

## Requirements

### Requirement: Commands
The system SHALL provide the commands `list`, `update`, `upgrade` and `help`, defaulting to help when no command is given.

**Source:** cli/src/args.rs:7, cli/src/args.rs:26

#### Scenario: No arguments show help
- **GIVEN** no arguments
- **WHEN** the CLI parses them
- **THEN** the help action is chosen

### Requirement: Options
The system SHALL accept `-y`/`--yes` (do not ask for confirmation), `-n`/`--dry-run` (only show what would run), `-a`/`--all` (list including dependencies), `--firmware`, `--only LIST`, `--skip LIST` and `-h`/`--help`; an unknown source in an only/skip list MUST be an error.

**Source:** cli/src/args.rs:33

#### Scenario: Unknown option is rejected
- **GIVEN** an unrecognised option starting with `-`
- **WHEN** the CLI parses it
- **THEN** it reports an unknown-parameter error

#### Scenario: Unknown source in a list is rejected
- **GIVEN** `--only` with a word that is not a source
- **WHEN** the CLI parses it
- **THEN** it reports an unknown-source error

### Requirement: Word classification
The system SHALL treat the first bare word as the command and later words as either a source, a `source:package` target or a package name, resolving synonyms; a `foo:bar` whose prefix is not a known source MUST be treated as a whole package name.

**Source:** cli/src/args.rs:61

#### Scenario: Known source word selects the source
- **GIVEN** the word `node`
- **WHEN** it is classified
- **THEN** it selects the `npm` source

#### Scenario: Qualified unknown prefix is a package name
- **GIVEN** the word `foo:bar` where `foo` is not a source
- **WHEN** it is classified
- **THEN** it is treated as a package named `foo:bar`

### Requirement: Legacy invocation
The system SHALL remain compatible with the previous script: options given without a command MUST be treated as an `upgrade`.

**Source:** cli/src/args.rs:92

#### Scenario: Options without a command mean upgrade
- **GIVEN** the invocation `--only npm -n` with no command word
- **WHEN** the CLI parses it
- **THEN** the action is `upgrade`

### Requirement: Root refusal
The system MUST refuse to run with superuser privileges, explaining that package managers would then act for the wrong user, and MUST require a `HOME` directory.

**Source:** cli/src/main.rs:29

#### Scenario: Running as root is refused
- **GIVEN** UPM is started as root
- **WHEN** it runs
- **THEN** it prints an explanation and exits non-zero without doing anything

### Requirement: Interruption handling
The system SHALL register handlers for interrupt and termination signals so that in-progress work stops, and SHALL exit with code 130 when interrupted.

**Source:** cli/src/main.rs:42, cli/src/render.rs:11

#### Scenario: Interrupt yields 130
- **GIVEN** an upgrade in progress
- **WHEN** the user sends an interrupt
- **THEN** work stops, a summary is printed and the exit code is 130

### Requirement: Output formatting
The system SHALL print aligned tables whose columns align by character count (including non-ASCII text), use colour only when writing to a terminal, send diagnostics and program output to the error stream, and exit successfully when the reader closes the pipe early.

**Source:** cli/src/output.rs:42, cli/src/output.rs:57, cli/src/output.rs:92

#### Scenario: Broken pipe is not an error
- **GIVEN** the output of `upm list` is piped into a reader that closes after the first line
- **WHEN** the CLI writes the next line
- **THEN** it exits successfully without an error

### Requirement: Exit codes
The system SHALL return 0 on success, a non-zero code when a named package is not found or a check failed, 2 on a usage error, and 130 on interruption.

**Source:** cli/src/main.rs:17, cli/src/render.rs:49

#### Scenario: Missing package changes nothing
- **GIVEN** an `upgrade` naming a package that cannot be found
- **WHEN** it runs
- **THEN** it reports the missing name, changes nothing, and exits non-zero

### Requirement: Update and upgrade reporting
The system SHALL, after `update`, show the available updates per source and a hint to run `upgrade`; and after `upgrade`, a summary of successful, skipped and failed sources, a note when nothing was actually changed in dry-run, the manual report when upgrading everything, and a hint that firmware is skipped unless requested.

**Source:** cli/src/render.rs:83, cli/src/render.rs:147, cli/src/render.rs:178

#### Scenario: Firmware hint on full upgrade
- **GIVEN** a full `upgrade` without `--firmware`
- **WHEN** it finishes
- **THEN** a hint explains how to include firmware

<!-- __inferspec_meta__: {"hash": "05869de8507b06241bfe787d64af293de2b5801c463a7802159e40d7dff41d4d", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
