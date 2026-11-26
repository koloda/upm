# Proposal

## Why

UPM knows only one distribution package manager, DNF, so on Debian, Ubuntu, Arch, openSUSE, Alpine or NixOS the most important part of the system is invisible. Supporting the popular managers, and showing only the ones that belong to the host, makes UPM useful on any mainstream Linux without cluttering it with irrelevant sources.

## What Changes

- Detect the host's distribution family and activate at most one native package family: RPM, openSUSE, Debian, Arch or Alpine. A manager binary that merely happens to be installed (for example `apt` on Fedora) does not activate its source.
- Add native sources: **APT** (Debian, Ubuntu and derivatives), **Pacman** and **AUR** (Arch and derivatives), **Zypper** (openSUSE, SLE), **APK** (Alpine), **YUM** (older RPM systems without DNF).
- Add a generic read-only **RPM** source for RPM systems where UPM has no manager it can safely drive (atomic/ostree systems, or no `dnf`, `yum` or `zypper`).
- Add distribution-independent sources: **Snap** and **Nix** (user profile), shown wherever they are installed. DNF, Flatpak and Homebrew remain.
- Each new source lists, checks, upgrades and removes through its own manager, except where that would be unsafe: no partial upgrades on Arch, no AUR builds without a terminal, no changes from the generic RPM source or to declarative NixOS and atomic system packages.
- **BREAKING**: DNF is offered only on RPM-family hosts that are not atomic.
- **BREAKING**: sources absent from this computer are no longer reported as "skipped" in `update` and `upgrade` unless the user names them.
- **BREAKING**: the words `apt`, `pacman`, `aur`, `apk`, `zypper`, `yum`, `rpm`, `snap`, `nix`, `deb` and `dpkg` now select sources; a package with such a name must be written as `source:name`. `rpm` selects the host's active RPM source instead of always DNF.
- Replace RPM-only assumptions (file ownership, orphan report, graphical password helpers) with ones that follow the host.
- macOS gets no new support: it shows no Linux native sources, and nothing else changes.

Out of scope: Portage, XBPS and other less common managers, `rpm-ostree` deployments, NixOS `configuration.nix` and Home Manager, `doas`, distribution packaging of UPM itself, and native macOS application inventory.

## Capabilities

### New Capabilities
- `platform-detection`: Identifying the host's distribution family and deciding which native source family applies.
- `distro-sources`: Behaviour of the APT, Pacman, AUR, Zypper, APK, YUM and generic RPM sources.
- `universal-sources`: Behaviour of the distribution-independent Snap and Nix sources.

### Modified Capabilities
- `source-registry`: New source ids, order and synonyms; host-aware availability and `rpm` synonym.
- `system-sources`: DNF is limited to non-atomic RPM-family hosts.
- `operation-engine`: Absent sources are silent; sudo is requested only for present sources; the orphan report follows the active native source.
- `command-environment`: Package-ownership test for every native family, Nix tool locations, more password helpers, manager-specific success exit codes.
- `cli-frontend`: `update` and `upgrade` report only relevant sources and explain skipped ones.
- `gui-frontend`: The sidebar shows only the host's sources, and the status reports skipped work.
- `language-sources`: npm skips files owned by any distribution package, not only RPM.
- `application-sources`: Self-updating programs are excluded when any distribution package owns them.
- `acceptance-tests`: The black-box suite covers each distribution family with a simulated host.

## Impact

- `common/src/`: new platform module, `context.rs`, `engine.rs`, `source.rs` only if needed, `sources/` (registry plus new modules per family), `sources/apps.rs` and `sources/lang.rs` (ownership).
- `cli/src/args.rs`, `cli/src/render.rs`; `gui/src/view.rs`, `gui/src/jobs.rs`.
- `tests/blackbox.sh`: stubs for the new managers and host fixtures.
- No new Rust dependencies. New optional runtime tools: `checkupdates` (pacman-contrib), `paru` or `yay`.
- Verifying real command output needs distribution containers (podman is available on the development machine).
- Main risks: destructive or partial-upgrade commands on systems that are not the development machine.
