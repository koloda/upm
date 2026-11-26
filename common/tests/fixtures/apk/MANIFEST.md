# Captured from a real system

- Image: `docker.io/library/alpine:3.20.0`
- Tool version: `apk-tools 2.14.4, compiled for x86_64.`
- Captured: 2026-11-26

| File | Exit | Command |
|---|---|---|
| info.txt | 0 | `apk info -v` |
| world.txt | 0 | `cat /etc/apk/world` |
| version-updates.txt | 0 | `apk version -l '<'` |
| owner.txt | 0 | `apk info --who-owns /bin/busybox` |
| owner-none.txt | 1 | `touch /tmp/nobody; apk info --who-owns /tmp/nobody` |
