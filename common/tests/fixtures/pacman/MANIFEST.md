# Captured from a real system

- Image: `docker.io/library/archlinux:base-20250601.0.358000 (fully upgraded, then tree and which downgraded; two locally built foreign packages)`
- Tool version: `pacman 7.1.0.r9.g54d9411-4 pacman-contrib 1.13.1-1 `
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| native-explicit.txt | 0 | `pacman -Qen` |
| native-all.txt | 0 | `pacman -Qn` |
| foreign-explicit.txt | 0 | `pacman -Qem` |
| foreign-all.txt | 0 | `pacman -Qm` |
| qun-updates.txt | 0 | `pacman -Qun` |
| checkupdates-updates.txt | 0 | `checkupdates` |
| owner.txt | 0 | `pacman -Qo /usr/bin/pacman` |
| owner-none.txt | 1 | `touch /tmp/nobody; pacman -Qo /tmp/nobody` |
