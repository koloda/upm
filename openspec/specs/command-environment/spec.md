## Purpose

Every external program UPM runs goes through a `Context`. It fixes the working directory to the user's home, extends `PATH` so tools are found from any launcher, honours dry-run and auto-yes modes, routes all output through the reporter, remembers failed commands and notes, supports cancellation, and handles sudo password prompts. This capability is the seam between the core and the operating system.

## Requirements

### Requirement: Launcher-independent environment
The context MUST build its own `PATH` that includes the user's usual tool locations (for example `~/.local/bin`, Linuxbrew, `~/go/bin`, the Composer vendor bin, the OpenCode and Mimocode bins, `~/.cargo/bin`, `~/.bun/bin`) so that results do not depend on whether UPM was started from fish, bash, cron or a desktop shortcut.

**Source:** common/src/context.rs:108

#### Scenario: Tools are found regardless of launcher
- **GIVEN** a tool installed in `~/.cargo/bin` that is absent from the launching shell's `PATH`
- **WHEN** the context checks for that tool
- **THEN** it is found

#### Scenario: Commands run from the home directory
- **GIVEN** any external command
- **WHEN** it is executed
- **THEN** its working directory is the user's home

### Requirement: Dry-run only shows commands
In dry-run mode the system MUST display each command it would run but MUST NOT execute it, and MUST report success.

**Source:** common/src/context.rs:250

#### Scenario: Nothing changes in dry-run
- **GIVEN** dry-run mode is enabled
- **WHEN** an upgrading command runs
- **THEN** the command is shown and the call reports success without running it

### Requirement: Capturing queries
The system SHALL provide a way to run a command and read its output for queries: stdout is captured, stderr is forwarded to the reporter when output is captured, and a command is not started at all once the run has been interrupted.

**Source:** common/src/context.rs:199, common/src/context.rs:244

#### Scenario: Query returns stdout
- **GIVEN** a command that prints data and exits successfully
- **WHEN** it is captured
- **THEN** the data is returned and a successful status is reported

#### Scenario: Interrupted run stops issuing queries
- **GIVEN** an interrupted run
- **WHEN** a query command is captured
- **THEN** the command is not started and empty output is returned

### Requirement: Streaming runs and failure memory
The system SHALL run commands, echo each command line to the reporter, and remember every failed command; a variant SHALL route a command's output to the error stream so it does not mix with results.

**Source:** common/src/context.rs:250, common/src/context.rs:267, common/src/context.rs:324

#### Scenario: Failed command is remembered
- **GIVEN** a command that exits with a non-zero code
- **WHEN** it is run
- **THEN** it is recorded among failed commands and reported as a failure

### Requirement: Cancellation
The system SHALL expose a shared cancellation flag set by the interface (Ctrl+C or a Cancel button); once set, no new commands are started.

**Source:** common/src/context.rs:144, common/src/context.rs:148

#### Scenario: Cancelled flag stops new work
- **GIVEN** the cancellation flag has been set
- **WHEN** the system is about to run a command
- **THEN** the command is not started

### Requirement: Sudo handling
The system SHALL ask for the sudo password through a graphical helper only for a `sudo` command that may prompt (not one already given `-n`), and SHALL elevate only when the current user cannot write to the target directory.

**Source:** common/src/context.rs:70, common/src/context.rs:339

#### Scenario: Non-prompting sudo is left alone
- **GIVEN** a `sudo -n` command
- **WHEN** the askpass helper is applied
- **THEN** the command is unchanged

#### Scenario: Locally writable directory needs no elevation
- **GIVEN** a directory the user can write to
- **WHEN** a command is run in that directory
- **THEN** it runs without sudo

### Requirement: No headless prompts
When output is captured, the system MUST NOT let a command wait for user input: interactive commands are detected from the absence of a terminal and their stdin is closed.

**Source:** common/src/context.rs:178

#### Scenario: Captured command never blocks on input
- **GIVEN** output is captured
- **WHEN** a command tries to read input
- **THEN** it sees end-of-input instead of blocking

### Requirement: Notes for the summary
The system SHALL let sources attach notes (for example manual-update reminders) that are both warned immediately and collected for the final summary.

**Source:** common/src/context.rs:365

#### Scenario: Note is collected
- **GIVEN** a source attaches a note
- **WHEN** the run finishes
- **THEN** the note is available in the run's notes list

### Requirement: File and package helpers
The system SHALL provide helpers to test whether a file is executable, list a directory's visible entries alphabetically, take a file name, and test whether a file belongs to an RPM package.

**Source:** common/src/context.rs:355, common/src/context.rs:414, common/src/context.rs:420, common/src/context.rs:433

#### Scenario: RPM-owned file is recognised
- **GIVEN** `rpm` is available and a file is owned by an RPM package
- **WHEN** the ownership test runs
- **THEN** the file is reported as RPM-owned

<!-- __inferspec_meta__: {"hash": "39417f79dad593032cf514cc2ead7a03f4446b8f42d7d9d8d9187a3961c2abc5", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
