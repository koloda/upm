# Captured from a real system

- Image: `docker.io/opensuse/leap:15.6`
- Tool version: `zypper 1.14.94`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| userinstalled.txt | 0 | `zypper --non-interactive packages --installed-only --userinstalled` |
| rpm-all.txt | 0 | `rpm -qa --qf '%{NAME} %{VERSION}-%{RELEASE}\n'` |
| owner.txt | 0 | `rpm -qf /usr/bin/zypper` |
| list-updates.txt | 0 | `zypper --non-interactive --no-refresh list-updates` |
