# Captured from a real system

- Image: `docker.io/opensuse/tumbleweed:latest`
- Tool version: `zypper 1.14.102`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| os-release.txt | 0 | `cat /etc/os-release` |
| userinstalled.txt | 0 | `zypper --non-interactive packages --installed-only --userinstalled` |
| list-updates.txt | 0 | `zypper --non-interactive --no-refresh list-updates` |
