# Spec Delta

## ADDED Requirements

### Requirement: Per-distribution black-box coverage
The black-box suite SHALL run the CLI against a simulated host for each supported native family and for an immutable host, independent of the distribution it runs on, and assert that only that host's sources are queried and that each native source issues the expected list, check and upgrade commands.

#### Scenario: Suite runs on any distribution
- **GIVEN** the suite runs on a Fedora development machine
- **WHEN** it simulates a Debian host
- **THEN** APT stubs are called and DNF stubs are not

#### Scenario: Safety rules are asserted
- **GIVEN** a simulated Arch host
- **WHEN** the suite upgrades one Pacman package
- **THEN** no mutating Pacman command is called
