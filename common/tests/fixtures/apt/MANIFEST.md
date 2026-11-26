# Captured from a real system

- Image: `docker.io/library/debian:bookworm-20240812-slim`
- Tool version: `apt 2.6.1 (amd64)`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| installed.tsv | 0 | `dpkg-query -W -f='${db:Status-Status}\t${Package}\t${Version}\t${Architecture}\n'` |
| manual.txt | 0 | `apt-mark showmanual` |
| native-arch.txt | 0 | `dpkg --print-architecture` |
| simulate-upgrade.txt | 0 | `apt-get -s upgrade --with-new-pkgs` |
| owner.txt | 0 | `dpkg-query -S /usr/bin/dpkg` |
| owner-none.txt | 1 | `touch /tmp/nobody && dpkg-query -S /tmp/nobody` |
| remove-essential.txt | 100 | `apt-get remove -y bash` |
| simulate-none.txt | 0 | `apt-get -s upgrade --with-new-pkgs` |
