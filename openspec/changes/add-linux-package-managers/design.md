# Design

## Context

Each kind of software is a `Source` (`common/src/source.rs`) listed in a fixed registry (`common/src/sources/mod.rs`). Both frontends already hide a source from `list` and the sidebar when `available()` is false, so auto-discovery is mostly a matter of making `available()` correct. Today it only checks that a tool is on `PATH`, which is wrong for distribution managers: Fedora can have `apt` and `dpkg` installed.

Other RPM assumptions: `Context::rpm_owned`, the orphan report in `engine::manual_report` (runs whenever `dnf` exists), Fedora-only askpass paths, and `check_updates`/`upgrade` printing every absent source (tolerable with 15 sources, noisy with 24). The development host is Fedora 45; other managers can only be exercised in containers.

## Goals / Non-Goals

**Goals:**
- One place decides what the host is; sources and frontends never re-detect it.
- One source per manager, behind the existing trait, with exact commands pinned by tests.
- Every host family testable on Fedora with stubs.

**Non-Goals:**
- Portage, XBPS, `rpm-ostree` transactions, NixOS configuration, Home Manager, `doas`.
- Building or packaging UPM for other distributions or macOS.
- A removal preview listing what a manager would cascade to.

## Decisions

### 1. A `platform` module, detected once
`Platform { family: Option<Family>, immutable: bool, rolling: bool }`, parsed from `/etc/os-release` (fallback `/usr/lib/os-release`), `/run/ostree-booted` and package database paths; macOS from the compile target. It is held by `Context` and tests set it directly. `UPM_HOST_ROOT=<dir>` prefixes every probed path (including the snapd socket) so the black-box suite can simulate hosts; it changes only which sources are active.

*Alternatives:* tool presence only (activates several families at once); a user-edited config file (the request is auto-discovery).

### 2. Gate in `available()`, keep the registry static
All 24 sources stay registered, so ids, ordering and "named but absent" messages keep working. The registry becomes `sources::all(&Platform)` only so that a source can be built read-only: Pacman on an immutable host must answer `can_remove() == false`, which has no context argument. `sources::removable` and `Catalog` receive the same platform.

*Alternative:* registering only the host's sources, rejected because `NAMES`, catalog order and CLI parsing would become host-dependent.

### 3. Separate ids per manager, files per family
New modules `sources/{rpm,deb,arch,alpine,universal}.rs`; DNF moves to `rpm.rs`; the duplicated `run_removals` helper moves to `sources/mod.rs`. YUM is its own source (its parsing differs) rather than a hidden DNF mode, so the UI names the real tool.

### 4. `rpm` stays a pure id; host mapping happens at input
`canonical("rpm")` returns `rpm`. The CLI then maps it to the first available of `dnf`, `yum`, `zypper`, `rpm` for source words, `--only`, `--skip` and `rpm:package`. The GUI passes real ids and needs no mapping.

### 5. Commands

Queries that are parsed run with `LC_ALL=C`. "yes" is the flag added only with auto-yes. Checks skip the refresh step in dry-run. Spellings were confirmed in containers or upstream documentation; captured output is in `common/tests/fixtures/`, each directory saying whether it comes from a real system.

| Source | List (default / all) | Check | Upgrade (all / selected) | Remove | yes |
|---|---|---|---|---|---|
| apt | `dpkg-query -W` (status, package, version, architecture) filtered by `apt-mark showmanual` / unfiltered | `sudo apt-get update`; `apt-get -s upgrade --with-new-pkgs` | `apt-get upgrade --with-new-pkgs` / `apt-get install --only-upgrade --no-remove` | `apt-get remove` | `-y` |
| pacman | `pacman -Qen` / `-Qn` | `checkupdates`, else or in dry-run `pacman -Qun` | `pacman -Syu` / skipped | `pacman -R` | `--noconfirm` |
| aur | `pacman -Qem` / `-Qm` | `<helper> -Qua` | `<helper> -Sua` / `<helper> -S` | `pacman -R` | `--noconfirm` |
| zypper | `zypper packages --installed-only --userinstalled` / `rpm -qa` | `sudo zypper refresh`; `zypper list-updates` | `zypper update` or `dup` / `zypper update` | `zypper remove` | `--non-interactive` |
| apk | `apk info -v` filtered by `/etc/apk/world` / unfiltered | `sudo apk update`; `apk version -l '<'` | `apk upgrade` / `apk upgrade <pkgs>` | `apk del` | omit `--interactive` |
| yum | `repoquery --cache --installed --all` with `%{yumdb_info.reason}` = `user`, else all / `rpm -qa` | `sudo yum check-update` (`-C` without sudo in dry-run) | `yum update` | `yum remove` | `-y` |
| rpm | `rpm -qa` | all unchecked | skipped | none | - |
| snap | `snap list` | `snap refresh --list` | `snap refresh` | `snap remove` | - |
| nix (profile) | `nix profile list --json` | unchecked | `nix profile upgrade --all` / names | `nix profile remove` | - |
| nix (env) | `nix-env -q --json` | `nix-env --upgrade --dry-run` | `nix-env --upgrade` | `nix-env --uninstall` | - |

Mutations of apt, pacman, zypper, apk, yum and snap use `sudo`; these sources report `needs_sudo`. Pacman and AUR removal need `sudo pacman`. AUR helpers are never run through `sudo`.

#### Observed behaviour that shapes the code
- `binary:Package` gains `:amd64` suffixes on multi-arch systems; keys are built from `Package` and `Architecture` (plain for native and `all`, `name:arch` otherwise), matching `apt-mark showmanual` and `Inst` lines.
- `apt-get install --only-upgrade` marks the named package as manually installed; packages that were automatic are restored with `sudo apt-mark auto`.
- APT and APK refresh their package lists before upgrading, as DNF's `--refresh` does.
- `yum history userinstalled` does not exist in yum 3.4.3. `repoquery` comes with `yum-utils`.
- `pacman -R`/`-S` without `--noconfirm` exits 0 without acting when stdin ends; `apk -i` *proceeds* when stdin ends. Pacman, AUR and APK changes are therefore skipped with a note when auto-yes is off and there is no terminal (dry-run still shows them).
- `apk del` exits 0 when it keeps a package that others require; removal is verified with `apk info -e` and reported as failed.
- `zypper --non-interactive remove` carries out cascades that take core packages (it removed `rpm` itself in a container).
- `nix-env --upgrade --dry-run` writes its `upgrading 'a-1' to 'a-2'` lines to stderr. `nix profile upgrade --dry-run` prints flake references without versions, so modern profiles stay unchecked.
- Zypper 102 (reboot) and 103 (package manager updated, run again) are successes with a note.
- Runtime snaps carry no note in `snap list`; they are recognised by the `base`, `core` and `snapd` notes plus the known runtime names (`bare`, `gtk-common-themes`, `gnome-*`, `kf5-*`, `kf6-*`, `mesa-*`).
- Immutable openSUSE hosts are recognised by ids containing `microos`, `leap-micro`, `sl-micro`, `sle-micro`, `aeon` or `kalpa`.

### 6. Safety rules over convenience
- **Pacman selected upgrade is skipped**, not widened to `-Syu`: upgrading the whole system when one package was ticked would surprise the user, and `-S pkg` after a sync is a partial upgrade.
- **AUR upgrade requires `ctx.interactive()`.** The window forces yes, which would build unreviewed PKGBUILDs. Alternative (`--noconfirm` plus `--sudoflags -A`) rejected.
- **APT** uses `upgrade --with-new-pkgs`, not `full-upgrade`, so it never removes. Headless runs use `sudo env DEBIAN_FRONTEND=noninteractive apt-get -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold ...`; sudo would otherwise drop the variable, and the askpass rewrite still sees `sudo` first.
- No `--allow-*`, `--force*` (other than the dpkg config-file choice above), `--nodeps` or `-Rdd`.

### 7. Small core extensions
- `Cmd::ok_codes(&[..])`, honoured by `record` (Zypper 102/103).
- `Context::distro_owned(path)` replaces `rpm_owned`: `rpm -qf`, `dpkg-query -S`, `pacman -Qo` or `apk info --who-owns` by family; false with no family.
- Nix profile bins join the extra `PATH`; askpass gains Debian, Arch and openSUSE locations.
- The engine tests availability before emitting `Started` or adding to the summary, silently skipping absent sources unless named or picked; `needs_sudo` counts only available sources. DNF orphan report runs only when the DNF source is available.
- `jobs::report` names skipped sources in the status.

### 8. Nix profile format
Chosen from the profile's manifest (`manifest.json` → `nix profile`, `manifest.nix` → `nix-env`), checking both `~/.nix-profile` and the XDG state location. `nix` commands pass the experimental-features flag so a default install works.

## Risks / Trade-offs

- [A targeted `zypper update <package>` marks a dependency as user-installed, and zypper 1.14 has no command to undo it] → Accepted as the manager's behaviour; a full update keeps the marks.
- [Removal with auto-yes can cascade to dependent packages in APT, Zypper, YUM, as DNF already does; Zypper protects little] → Manager protections stay on, commands and output are logged, the cascade is documented. A preview is a recommended follow-up.
- [Parsers built against stubs drift from real output] → Fixtures captured from real containers; unverified commands are reported, not assumed.
- [Snap, SteamOS and atomic hosts cannot be fully run in containers] → Test parsers from captured or documented output and record what was never run on a real system.
- [GUI sudo has no password helper on a minimal install] → Existing behaviour: the command fails and appears in the log.
- [Pacman rows look selectable but selected upgrade is skipped] → Status and note say what to do instead.
- [`checkupdates` absent shows stale results] → Diagnostic says so.
- [New source words shadow package names such as `snap` or `nix`] → `source:name` still works; stated as breaking in the proposal.

## Migration Plan

No stored data. Fedora behaviour is unchanged apart from quieter output for absent sources and the new source words. Rollback is reverting the change.

## Verification record

Built as a static binary and run in containers as a non-root user with sudo (removal through a small driver over `engine::remove`, as the CLI has no delete command):

| Manager | Exercised on a real system |
|---|---|
| dnf | `list`, `update -n` on the Fedora 45 development host (read-only) |
| apt | list, update, targeted upgrade, removal, essential-package refusal (Debian) |
| pacman | list, update, refused selected upgrade, full upgrade, removal, dependency refusal (Arch) |
| zypper | list, update, targeted upgrade, removal (openSUSE Leap); `dup` run directly on Tumbleweed |
| apk | list, update, targeted upgrade, removal, kept-package failure (Alpine) |
| yum | list, update, targeted upgrade, removal (CentOS 7) |
| nix | legacy list, preview, removal; modern list, upgrade, removal |
| aur (`paru`/`yay`) | **never run**: building helpers from the AUR was not permitted; only Pacman-based listing ran |
| snap | **never run** |
| generic rpm, SteamOS, atomic and MicroOS hosts | **never run**; simulated in tests only |
| graphical interface on another distribution | **never run** |

