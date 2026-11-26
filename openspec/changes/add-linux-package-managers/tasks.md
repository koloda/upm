# Tasks

## 1. Capture real manager behaviour

- [ ] 1.1 In throwaway podman containers (Debian, Ubuntu, Arch, openSUSE Leap and Tumbleweed, Alpine, CentOS 7, Nix) run every query from design.md decision 5, with updates pending where possible, and save outputs and exit codes under `common/tests/fixtures/<manager>/`; verify each fixture records the image, tool version and command.
- [x] 1.2 Confirm every mutating command and option spelling (including `apk upgrade <pkgs>`, `zypper packages --userinstalled`, `paru`/`yay -Qua` and both Nix profile formats) by help text or by running it in a container, and verify design.md's table is corrected wherever reality differs.
- [x] 1.3 For what containers cannot exercise (Snap daemon, SteamOS, ostree hosts), take fixtures from upstream documentation or a VM, and verify each such fixture is labelled as not captured from a real system.

## 2. Platform detection and core plumbing

- [x] 2.1 Add `common/src/platform.rs` (family, immutable and rolling flags, fallback probing, `UPM_HOST_ROOT`, macOS) and hold it in `Context`; verify unit tests cover every scenario in the `platform-detection` spec, including Fedora with `apt` installed, a derivative via `ID_LIKE`, an unknown id and a missing os-release.
- [x] 2.2 Add declared success exit codes to `Cmd`; verify unit tests show 102 succeeds when declared and 1 is still recorded as failed.
- [x] 2.3 Replace `rpm_owned` with a family-aware ownership test used by the tools and npm sources; verify a unit test pins the query per family and returns false without a family.
- [x] 2.4 Add Nix profile bins to `PATH` and the extra askpass locations; verify unit tests cover discovery and that `SUDO_ASKPASS` wins.
- [x] 2.5 Make the engine silently skip absent unnamed sources in check and upgrade, request sudo only for present sources, and gate the orphan report on the DNF source; verify engine tests cover absent, named-absent, no-sudo and non-DNF cases.

## 3. Registry and RPM family

- [x] 3.1 Move shared removal helpers to `sources/mod.rs`, move DNF to `sources/rpm.rs` and gate it on the platform; verify existing DNF tests pass and new tests show it unavailable on Debian and on an ostree host.
- [x] 3.2 Make the registry and `removable` take the platform and have `Catalog` use it; verify catalog tests still pass and a read-only source reports its rows as not removable.
- [x] 3.3 Add the YUM source (including wrapped `check-update` lines); verify tests against its fixtures pin listing, check, upgrade, removal, dry-run and that it is unavailable when `dnf` exists.
- [x] 3.4 Add the Zypper source (rolling vs fixed upgrade, userinstalled fallback, 102/103 notes); verify tests against its fixtures cover each `distro-sources` Zypper scenario.
- [x] 3.5 Add the read-only generic RPM source and the host mapping of the word `rpm`; verify tests show no mutating command and `rpm` resolving to dnf, yum, zypper or rpm per host.

## 4. Debian, Arch and Alpine sources

- [x] 4.1 Add the APT source, including multi-arch keys and the headless environment and dpkg options; verify tests against its fixtures cover default and full listing, check, full and targeted upgrade, removal, auto-yes on and off, and dry-run.
- [x] 4.2 Add the Pacman source (`checkupdates` fallback, skipped selected upgrade, read-only on immutable hosts); verify tests show no `-Sy` without `-u`, no command for a selected upgrade, and a note explaining it.
- [x] 4.3 Add the AUR source (helper preference, no helper, terminal requirement); verify tests show nothing is built without a terminal, `--noconfirm` appears only with auto-yes and no helper runs through `sudo`.
- [x] 4.4 Add the APK source; verify tests against its fixtures cover the world filter, check, targeted upgrade leaving the world set alone, removal and confirmation behaviour.

## 5. Distribution-independent sources

- [x] 5.1 Add the Snap source with daemon detection and base-snap filtering; verify tests cover unavailable daemon, default and full listing, check, targeted refresh and removal.
- [x] 5.2 Add the Nix source for both profile formats; verify tests show the formats are never mixed, no `sudo` is used and a modern profile reports unchecked.
- [x] 5.3 Finalise the registry order, `NAMES`, synonyms and removal tests for all 24 sources; verify `names_match_registry` and synonym tests pass.

## 6. Command line and window

- [x] 6.1 Apply host mapping in CLI argument parsing and update `usage()` with the new sources and synonyms; verify `args` tests and that `upm help` lists them.
- [x] 6.2 Show titles for the new sources and name skipped sources in the GUI job status; verify `cargo test -p upm-gui` covers the titles, a skipped upgrade and a plain success.
- [x] 6.3 Add QML tests for a sidebar containing only host sources and for the skipped status; verify `QT_QPA_PLATFORM=offscreen qmltestrunner -input gui/tests` passes.

## 7. Black-box coverage

- [x] 7.1 Make the existing suite use a simulated Fedora host through `UPM_HOST_ROOT`; verify all current cases pass unchanged apart from absent-source output.
- [x] 7.2 Add Debian, Arch, openSUSE, Alpine, legacy YUM, NixOS and ostree host cases with stubs for each new manager (extending the mutating-command pattern); verify the suite asserts only that host's sources are called and the Arch selected-upgrade safety rule.

## 8. Integration verification

- [x] 8.1 Run `cargo test --workspace`, `tests/blackbox.sh target/debug/upm` and the QML tests; verify all pass.
- [x] 8.2 Run the built CLI on this Fedora host (`list`, `update -n`); verify only DNF appears among native sources and no new section shows for absent tools.
- [x] 8.3 In Debian, Arch, openSUSE and Alpine containers run `upm list`, `update`, and a targeted upgrade and removal of a disposable package as a non-root user with sudo; verify results match the specs and record which managers were not exercised on a real system.
