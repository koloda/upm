# Captured from a real system

- Image: `docker.io/nixos/nix:latest (profiles in /tmp/legacy and /tmp/modern built from nixpkgs 23.11)`
- Tool version: `nix (Nix) 2.36.0`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| env-query.json | 0 | `nix-env -p /tmp/legacy -q --json` |
| env-upgrade-dry.txt | 0 | `timeout 500 nix-env -p /tmp/legacy -f https://github.com/NixOS/nixpkgs/archive/refs/tags/24.05.tar.gz --upgrade --dry-run` |
| profile-list.json | 0 | `nix --extra-experimental-features "nix-command flakes" profile list --profile /tmp/modern --json` |
| env-upgrade-dry.txt | 0 | `timeout 500 nix-env -p /tmp/legacy -f https://github.com/NixOS/nixpkgs/archive/refs/tags/24.11.tar.gz --upgrade --dry-run 2>&1 \| grep -v '^trace:'` |

Nixpkgs `evaluation warning:` lines were removed from `env-upgrade-dry.txt`.
