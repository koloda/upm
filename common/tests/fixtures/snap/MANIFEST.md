# NOT captured from a real system

The snap daemon cannot run in the capture containers. Samples follow the official documentation:
https://snapcraft.io/docs/how-to-guides/manage-snaps/manage-updates/

- `list.txt`: `snap list` (columns from the documentation; base, snapd and runtime rows added in the same format).
- `refresh-list.txt`: `snap refresh --list`.
- With nothing to refresh, `All snaps up to date.` is printed and the table is empty.
