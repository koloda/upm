# Captured from a real system

- Image: `docker.io/library/centos:7 (repositories pointed at vault.centos.org)`
- Tool version: `3.4.3`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| os-release.txt | 0 | `cat /etc/os-release` |
| rpm-all.txt | 0 | `rpm -qa --qf '%{NAME} %{VERSION}-%{RELEASE}\n'` |
| check-update.txt | 100 | `yum check-update` |
| check-update-cached.txt | 100 | `yum check-update -C` |
| repoquery-reason.txt | 0 | `repoquery --cache --installed --all --qf '%{name} %{yumdb_info.reason}'` |
