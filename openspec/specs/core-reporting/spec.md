## Purpose

The core of UPM never prints anything itself. Everything it has to say — progress, warnings, out-of-band diagnostics, raw program output, the command about to run, and failures — goes through one `Reporter` channel. The terminal and the graphical interface each implement that channel in their own way. This keeps the core free of presentation concerns.

## Requirements

### Requirement: Single reporting channel
The system SHALL route all core output through a reporter that exposes progress information, warnings, out-of-band diagnostics, raw log text, the command about to run (with a flag for side-effect commands), and failures.

**Source:** common/src/report.rs:4

#### Scenario: Interface receives core messages
- **GIVEN** a reporter implementation attached to a running operation
- **WHEN** the core emits progress and warnings
- **THEN** they arrive at the reporter instead of being printed by the core

### Requirement: Result versus side channels
The system SHALL distinguish messages that are part of the result (progress and warnings) from out-of-band diagnostics, which MUST NOT be presented as part of the result.

**Source:** common/src/report.rs:5, common/src/report.rs:11

#### Scenario: Diagnostic is kept out of the result
- **GIVEN** the core emits a diagnostic during a query
- **WHEN** the interface presents the result
- **THEN** the diagnostic is shown as a side note, not as part of the result

### Requirement: No-terminal detection
The system SHALL let a reporter declare that it captures output, meaning there is no terminal, so that dependent programs' output must be forwarded as log text and their questions will go unanswered.

**Source:** common/src/report.rs:25

#### Scenario: Capturing reporter means no terminal
- **GIVEN** a reporter that captures output
- **WHEN** the core decides whether an interactive program may prompt
- **THEN** it treats the environment as headless

### Requirement: Reporting thread safety
The reporter MUST be usable from more than one thread.

**Source:** common/src/report.rs:4

#### Scenario: Background work can report
- **GIVEN** an operation running on a background thread
- **WHEN** it emits a message
- **THEN** delivery to the reporter is allowed

<!-- __inferspec_meta__: {"hash": "6c3dd463db112486ebaea67472b19c3110ce598bc59d16ac2b411383f4d76a53", "scan_ts": "2026-11-26T14:21:18Z", "version": "0.1"} -->
