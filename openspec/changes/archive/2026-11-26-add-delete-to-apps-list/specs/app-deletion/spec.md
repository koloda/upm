# Spec Delta

## Purpose

Lets users remove installed software from the graphical apps list using the same manager or tool that installed it, with explicit selection, confirmation, and a refresh of the affected inventory afterwards.

## ADDED Requirements

### Requirement: Removal goes through the owning manager
The system MUST uninstall software by invoking the manager or tool of the source that installed it. A source whose items have no native uninstall command MUST NOT delete files itself and MUST be reported as not removable; firmware MUST never be removable.

#### Scenario: A package is uninstalled by its manager
- **WHEN** the user removes a selected package from a source that supports removal
- **THEN** that source's uninstall command is invoked for that package and no file is deleted directly

#### Scenario: A source without an uninstall command is not removable
- **WHEN** the user opens removal for an item whose source has no native uninstall
- **THEN** the item is reported as not removable and no uninstall or file deletion is attempted

### Requirement: Removal is targeted only
The system MUST remove only explicitly selected items. It MUST NOT offer whole-source, whole-category or global removal, and an empty, stale or unsupported selection MUST remove nothing.

#### Scenario: Only selected items are removed
- **GIVEN** two visible packages
- **WHEN** the user removes one of them
- **THEN** only that package is uninstalled and the other is left untouched

#### Scenario: A stale selection removes nothing
- **GIVEN** a selection whose package is no longer in the inventory
- **WHEN** the removal is requested
- **THEN** no item is uninstalled and the request is reported as having nothing to remove

### Requirement: Confirmation before uninstalling
The system MUST ask for confirmation before any uninstall, naming each item and its source. The confirmation MUST be non-confirmable when nothing can be removed, and cancelling it MUST leave the system unchanged.

#### Scenario: Confirmation lists the exact items
- **GIVEN** a selection of removable packages
- **WHEN** the user requests removal
- **THEN** a confirmation dialog lists each selected package together with its source

#### Scenario: Nothing to remove cannot be confirmed
- **GIVEN** a selection containing only unsupported items
- **WHEN** the confirmation dialog opens
- **THEN** its confirm action is disabled and no uninstall is possible

#### Scenario: Cancelling changes nothing
- **GIVEN** an open removal confirmation
- **WHEN** the user cancels it
- **THEN** no uninstall command is run and the selection is preserved

### Requirement: Dry run, sudo and interruption
Removal MUST respect dry-run mode by showing the commands it would run without changing anything, MUST obtain sudo credentials up front when any selected item needs them, and MUST stop on interruption without starting further items.

#### Scenario: Dry run does not uninstall
- **GIVEN** a removal request in dry-run mode
- **WHEN** the removal runs
- **THEN** the intended uninstall commands are shown and the software stays installed

#### Scenario: Interruption stops the remaining items
- **GIVEN** a removal of several packages
- **WHEN** the user interrupts it
- **THEN** no further package is uninstalled and the result reports the interruption

### Requirement: Uninstall uses the rights of the installation prefix
When an item is installed into a directory the current user cannot write to, the system MUST run that item's uninstall command with elevated privileges, exactly as its upgrade command is run. It MUST NOT drop privileges, change the recorded prefix, or turn an elevation failure into a silent success.

#### Scenario: Root-owned global npm prefix
- **GIVEN** a global npm package installed under `/usr/local/lib/node_modules`
- **WHEN** the user removes that package
- **THEN** the uninstall runs through `sudo` with the recorded `--prefix /usr/local`, so no `EACCES` failure is reported

#### Scenario: User-owned prefix needs no elevation
- **GIVEN** a global npm package installed under a directory the user can write to
- **WHEN** the user removes that package
- **THEN** the uninstall runs without `sudo`

### Requirement: Removal result and refresh
The system MUST report each item as removed, skipped or failed, MUST surface failed commands in the log, and MUST refresh the inventory of every affected source after a completed removal so removed items disappear from the list.

#### Scenario: Removed items disappear from the apps list
- **GIVEN** a package that was shown in the apps list
- **WHEN** its removal completes successfully
- **THEN** the affected source is reloaded and the package is no longer shown

#### Scenario: A failed uninstall is reported
- **GIVEN** an uninstall command that fails
- **WHEN** the removal finishes
- **THEN** the failure appears in the log and the result marks that item as failed
