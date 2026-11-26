# NOT captured from a real system

AUR helpers were not run: building them from the AUR was not permitted during capture.
Lines follow the format printed by upstream sources.

- `qua-paru.txt`: `print_upgrade` in https://github.com/Morganamilo/paru/blob/master/src/query.rs
  (`name old -> new`, optional ` [ignored]`; exit 1 when there are no updates).
- `qua-yay.txt`: pacman-style `name old -> new` upgrade lines (assumed from yay extending `-Qu`,
  https://github.com/Jguer/yay/blob/next/doc/yay.8).
- Options `-a/--aur` (hence `-Qua`, `-Sua`) are documented in both manuals.
