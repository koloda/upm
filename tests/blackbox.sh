#!/usr/bin/env bash
# Чорно-скринькові тести list/update/upgrade. Усі менеджери пакетів — заглушки,
# HOME — підроблений, тож на справжній системі нічого не змінюється.
# Комп'ютер симулюється через UPM_HOST_ROOT та власні заглушки менеджерів, тому
# набір не залежить від дистрибутива, на якому запущений.
# Використання: tests/blackbox.sh <шлях до update-all.sh або бінарника upm>
SCRIPT=$(realpath "${1:?вкажіть програму для перевірки}")
W=$(mktemp -d "${TMPDIR:-/tmp}/upm-test.XXXXXX")
# B — спільні заглушки; N/<комп'ютер> — менеджери конкретного дистрибутива.
H=$W/home B=$W/stubs N=$W/native R=$W/roots SYS=$W/system LOG=$W/calls.log
mkdir -p "$H" "$B" "$N" "$R" "$SYS"

root() { # root <комп'ютер> <рядки os-release> [файли відносно кореня...]
  mkdir -p "$R/$1/etc" "$N/$1"; printf '%b' "$2" >"$R/$1/etc/os-release"
  local name=$1 file; shift 2
  for file; do mkdir -p "$(dirname "$R/$name/$file")"; : >"$R/$name/$file"; done
}
root fedora 'ID=fedora\n'
PASS=0 FAIL=0

stub() { # stub <шлях> ; тіло зі stdin
  mkdir -p "$(dirname "$1")"
  { echo '#!/bin/bash'; echo "echo \"\$(basename \"\$0\") \$*\" >>'$LOG'"; cat; } >"$1"
  chmod +x "$1"
}

stub "$B/sudo" <<'E'
case $1 in -v|-n) exit 0 ;; esac
# Запити менеджерів ідуть через sudo — виконуємо їх заглушки.
case $1 in dnf|yum) exec "$@" ;; esac
exit 0
E
stub "$N/fedora/dnf" <<'E'
case "$*" in
  *repoquery*) printf 'bash 5.3-1\nfirefox 140-1\n' ;;
  *check-upgrade*) printf 'Upgrades\nfirefox.x86_64     141-1   updates\nlibdep.x86_64   2-1   updates\n\nObsoleting packages\nnewthing.x86_64 1 updates\n    oldthing.x86_64 1 @System\n'; exit 100 ;;
esac
E
stub "$N/fedora/rpm" <<'E'
[[ $1 == -qf ]] && exec /usr/bin/rpm "$@"
all=$'bash 5.3-1\nfirefox 140-1\nlibdep 1-1'
[[ $1 == -qa ]] && { echo "$all"; exit; }
name=${!#}; awk -v n="${name%.*}" '$1==n{print $2; f=1} END{exit !f}' <<<"$all"
E
stub "$B/flatpak" <<'E'
case "$*" in
  list*--app*) printf 'com.microsoft.Edge\t158\tMicrosoft Edge\tuser\n' ;;
  list*) printf 'com.microsoft.Edge\t158\tMicrosoft Edge\tuser\norg.gnome.Platform\t\tGNOME\tsystem\norg.gnome.Platform\t\tGNOME\tsystem\ncom.same.App\t1.0\tSame\tsystem\n' ;;
  remote-ls*--user*) printf 'com.microsoft.Edge\t159\tstable\n' ;;
  remote-ls*--system*) printf 'com.same.App\t1.0\tstable\n' ;;
esac
E
stub "$B/brew" <<'E'
[[ $1 == upgrade && -f $HOME/interrupt-brew ]] && { kill -INT 0; exit 130; }
case "$*" in
  leaves*) echo glab ;;
  "list --formula --versions") printf 'glab 1.0\nlibx 2.0\n' ;;
  "outdated --json=v2") echo '{"formulae":[{"name":"glab","installed_versions":["1.0"],"current_version":"1.1"}],"casks":[]}' ;;
esac
E
stub "$B/app-manager" <<'E'
[[ $1 == --update-check ]] && echo 'Bruno: 3.5.0 -> 3.6.0'
exit 0
E
stub "$B/npm" <<'E'
case "$1 $2" in
  "view fake-cli") echo 2.0.0 ;;
  "view @scope/tool") echo 1.5.0 ;;
  view*) echo 0.0.0 ;;
  "config get") echo "$HOME/.npm-global" ;;
esac
E
stub "$B/composer" <<'E'
case "$*" in
  *outdated*) echo '{"installed":[{"name":"laravel/installer","version":"v5.32.0","latest":"v5.33.0"}]}' ;;
  *show*) echo '{"installed":[{"name":"laravel/installer","version":"v5.32.0"}]}' ;;
esac
E
stub "$B/go" <<'E'
# Прапорець "interrupt-go": імітуємо Ctrl+C під час запиту останньої версії.
# "kill -INT 0" — уся група процесів, як від термінала; групу створює timeout у run().
[[ "$1 $2" == "list -m" && -f $HOME/interrupt-go ]] && kill -INT 0
case "$1 $2" in
  "env GOBIN") echo ;;
  "env GOPATH") echo "$HOME/go" ;;
  "version -m") n=${3##*/}; printf '%s: go1.26\n\tpath\texample.com/%s\n\tmod\texample.com/%s\tv1.0.0\th1:x\n' "$3" "$n" "$n" ;;
  "list -m") echo v1.1.0 ;;
esac
E
stub "$B/cargo" <<'E'
case "$*" in
  "install --list") printf 'ripgrep v14.0.0:\n    rg\n' ;;
  search*) echo 'ripgrep = "15.0.0"    # search tool' ;;
esac
E
stub "$B/uv" <<'E'
case "$*" in
  "tool list --outdated") printf 'ruff v0.1.0 [latest: 0.2.0]\n- ruff\n' ;;
  "tool list") printf 'ruff v0.1.0\n- ruff\n' ;;
esac
E
cat >"$B/python3" <<E
#!/bin/bash
if [[ " \$* " == *" -m pip "* ]]; then
  echo "python3 \$*" >>'$LOG'
  [[ " \$* " == *" list "* ]] && echo '[{"name":"mytool","version":"1.0","latest_version":"1.2"},{"name":"somedep","version":"1.0","latest_version":"9"}]'
  exit 0
fi
exec /usr/bin/python3 "\$@"
E
chmod +x "$B/python3"
stub "$B/fish" <<'E'
exit 0
E
stub "$B/gh" <<'E'
exit 0
E
stub "$B/fwupdmgr" <<'E'
case "$*" in
  "get-devices --json") echo '{"Devices":[{"Name":"SSD","DeviceId":"abc123","Version":"1.0","Flags":["updatable"]}]}' ;;
  "get-updates --json") echo '{"Devices":[{"Name":"SSD","DeviceId":"abc123","Version":"1.0","Releases":[{"Version":"1.1"}]}]}' ;;
esac
E
stub "$B/curl" <<'E'
[[ "$*" == *candidates/default/java* ]] && printf '25-tem'
exit 0
E

# --- підроблений HOME
mkpkg() { mkdir -p "$1"; printf '{"name":"%s","version":"%s"}\n' "$2" "$3" >"$1/package.json"; }
mkpkg "$H/.npm-global/lib/node_modules/fake-cli" fake-cli 1.0.0
mkpkg "$H/.local/lib/node_modules/@scope/tool" @scope/tool 1.0.0
mkdir -p "$H/go/bin"; for n in lazygit air; do printf '#!/bin/sh\n' >"$H/go/bin/$n"; chmod +x "$H/go/bin/$n"; done
mkdir -p "$H/.composer" "$H/.cargo"
pyver=$(/usr/bin/python3 -I -c 'import sys; print("%d.%d" % sys.version_info[:2])')
mkdist() { mkdir -p "$1/$2-$3.dist-info"; printf 'Name: %s\nVersion: %s\n' "$2" "$3" >"$1/$2-$3.dist-info/METADATA"; [[ -n ${4:-} ]] && touch "$1/$2-$3.dist-info/REQUESTED"; }
mkdist "$H/.local/lib/python$pyver/site-packages" mytool 1.0 req
mkdist "$H/.local/lib/python$pyver/site-packages" somedep 1.0
mkdist "$H/.local/lib/python3.1/site-packages" oldtool 0.9 req
mkdir -p "$H/.local/share/app-manager"
echo "{\"installations\":[{\"name\":\"Bruno\",\"version\":\"3.5.0\",\"installed_path\":\"$H/Applications/Bruno\"}]}" >"$H/.local/share/app-manager/installations.json"
stub "$H/.local/bin/droid" <<'E'
case "$*" in
  --version) echo 0.1.0 ;;
  "update --check") printf 'Current version: 0.1.0\n! Update available: 0.2.0\n' ;;
esac
E
stub "$H/.local/bin/claude" <<'E'
[[ $1 == --version ]] && echo '2.0.0 (Claude Code)'
exit 0
E
mkdir -p "$H/.sdkman/bin" "$H/.sdkman/candidates/java/21-tem"
ln -s 21-tem "$H/.sdkman/candidates/java/current"
cat >"$H/.sdkman/bin/sdkman-init.sh" <<E
export SDKMAN_CANDIDATES_API=https://api.invalid/2
sdk() { echo "sdk \$*" >>'$LOG'; }
E
stub "$H/Android/Sdk/cmdline-tools/latest/bin/sdkmanager" <<'E'
case $1 in
  --list|--list_installed) printf 'Installed packages:\n  build-tools/35.0.0      35.0.0                 Build-Tools\n  platform-tools          37.0.1    ->   37.0.3  Platform-Tools\n\nAvailable packages:\n  build-tools/99.0.0      99.0.0   Future\n' ;;
esac
E
mkdir -p "$H/.config/fish/functions"; touch "$H/.config/fish/functions/fisher.fish"
echo 'jorgebucaran/fisher' >"$H/.config/fish/fish_plugins"

mkdir -p "$H/Apps/Scripts/tool-source/src" "$H/Apps/SomeIDE/bin"
printf '#!/bin/sh\n' >"$H/Apps/Scripts/run-me.sh"; touch "$H/Apps/SomeIDE/product-info.json"

# --- допоміжне
# Поточний симульований комп'ютер та його PATH.
host() { HOST=$1; if [[ $1 == fedora ]]; then HOSTPATH=$N/fedora:$B:/usr/bin:/bin; else HOSTPATH=$N/$1:$B:$SYS; fi; }
host fedora
run() { : >"$LOG"; OUT=$(env -i HOME="$H" PATH="$HOSTPATH" UPM_HOST_ROOT="$R/$HOST" LANG=C.UTF-8 timeout 120 "$SCRIPT" "$@" </dev/null 2>&1); RC=$?; CALLS=$(cat "$LOG"); }
ok() { PASS=$((PASS + 1)); }
bad() { FAIL=$((FAIL + 1)); echo "FAIL [$T] $*"; }
has() { grep -qE -- "$1" <<<"$OUT" && ok || bad "у виводі немає: $1"; }
hasnt() { grep -qE -- "$1" <<<"$OUT" && bad "зайве у виводі: $1" || ok; }
called() { grep -qE -- "$1" <<<"$CALLS" && ok || bad "не викликано: $1"; }
notcalled() { grep -qE -- "$1" <<<"$CALLS" && bad "викликано зайве: $(grep -E -- "$1" <<<"$CALLS" | head -3)" || ok; }
rc() { [[ $RC == "$1" ]] && ok || bad "код $RC, очікувався $1; вивід: $(tail -5 <<<"$OUT")"; }
rcfail() { [[ $RC != 0 ]] && ok || bad "очікувався ненульовий код"; }
# Будь-яка команда, що змінює встановлене.
MUTATING='^(sudo (dnf upgrade|npm install|.* (update|selfupdate|self-update))|dnf upgrade|flatpak update|brew upgrade|app-manager (update|--update-all)|npm install|composer global (update|require)|go install|cargo install [^-]|uv tool upgrade|python3 .*pip install|fish .*fisher update|droid update$|claude update|sdk (upgrade|install|selfupdate)|sdkmanager (--update|[^-])|fwupdmgr update)'

T="без команди — довідка"; run; rc 0; has 'list'; has 'update'; has 'upgrade'; notcalled "$MUTATING"
T="невідома команда"; run frobnicate; rcfail; notcalled "$MUTATING"

T="list усе"; run list; rc 0; hasnt '==> (yum|zypper|rpm|apt|pacman|aur|apk|snap|nix) '
for p in firefox com.microsoft.Edge glab Bruno fake-cli @scope/tool laravel/installer lazygit ripgrep mytool ruff droid claude java platform-tools jorgebucaran/fisher; do has "$p"; done
hasnt 'libdep'; hasnt 'somedep'; hasnt 'libx'; hasnt 'org.gnome.Platform'; hasnt 'build-tools/99'; notcalled "$MUTATING"
T="list: версії"; has 'firefox +140-1'; has 'fake-cli +1\.0\.0'; has 'lazygit +v1\.0\.0'; has 'droid +0\.1\.0'; has 'oldtool'
T="list npm"; run list npm; rc 0; has fake-cli; has '@scope/tool'; hasnt firefox; hasnt lazygit; notcalled '^(dnf|brew|flatpak)'
T="list node (синонім)"; run list node; rc 0; has fake-cli; hasnt firefox
T="list --only"; run list --only go; rc 0; has lazygit; hasnt fake-cli
T="list --skip"; run list --skip dnf; rc 0; hasnt firefox; has fake-cli
T="list --all dnf"; run list --all dnf; rc 0; has libdep
T="list --all: дві гілки одного runtime — два рядки"; run list flatpak --all; rc 0; has '^==> flatpak \(4\)'
T="list --all brew"; run list brew --all; rc 0; has libx
T="list пакунок"; run list lazygit; rc 0; has lazygit; hasnt ' air '; hasnt firefox
T="list знаходить залежність"; run list libdep; rc 0; has libdep
T="list пошук за частиною назви"; run list fire; rc 0; has firefox; hasnt glab
T="list джерело:пакунок"; run list dnf:bash; rc 0; has 'bash +5\.3-1'; hasnt firefox
T="list пакунок у вибраному джерелі"; run list dnf fire; rc 0; has firefox; hasnt fake-cli
T="list нема такого"; run list zzznothing; rcfail

T="update усе"; run update; rc 0; hasnt '==> (yum|zypper|rpm|apt|pacman|aur|apk|snap|nix)$'
has 'firefox +140-1 +141-1'; has 'glab +1\.0 +1\.1'; has 'com\.microsoft\.Edge +158 +159'; has 'Bruno +3\.5\.0 +3\.6\.0'
has 'fake-cli +1\.0\.0 +2\.0\.0'; has 'laravel/installer +v5\.32\.0 +v5\.33\.0'; has 'lazygit +v1\.0\.0 +v1\.1\.0'; has 'ripgrep +14\.0\.0 +15\.0\.0'
has 'mytool +1\.0 +1\.2'; has 'ruff +0\.1\.0 +0\.2\.0'; has 'droid +0\.1\.0 +0\.2\.0'; has 'java +21-tem +25-tem'; has 'platform-tools +37\.0\.1 +37\.0\.3'
hasnt somedep; hasnt oldthing; hasnt 'build-tools/35'
called '^sudo dnf check-upgrade --refresh'; called '^brew update'; called '^sdk update'; notcalled "$MUTATING"
T="update flatpak: та сама версія — нова збірка; видно, де встановлено"; run update flatpak; rc 0
has 'com\.same\.App +1\.0 +нова збірка \[stable\] +system'; has 'com\.microsoft\.Edge +158 +159 \[stable\] +user'
T="update go"; run update go; rc 0; has lazygit; has air; hasnt firefox; notcalled '^(sudo|dnf|brew|npm)'; notcalled "$MUTATING"
T="update пакунок"; run update firefox; rc 0; has 'firefox +140-1 +141-1'; hasnt libdep; hasnt glab; notcalled "$MUTATING"
T="update актуальний пакунок"; run update bash; rc 0; has 'актуальн'
T="update нема такого"; run update zzznothing; rcfail
T="update -n"; run update -n go; rc 0; notcalled "$MUTATING"

T="upgrade go-пакунок"; run upgrade lazygit; rc 0; called '^go install example.com/lazygit@latest'; notcalled 'go install example.com/air'; notcalled '^(sudo|dnf upgrade|brew (update|upgrade)|npm install|flatpak update)'
T="upgrade npm:пакунок"; run upgrade npm:fake-cli; rc 0; called "^npm install -g --prefix $H/.npm-global fake-cli@2.0.0"; notcalled 'install .*@scope/tool'; notcalled '^(go install|sudo)'
T="upgrade коротка назва npm"; run upgrade tool; rc 0; called 'npm install .*@scope/tool@1.5.0'; notcalled 'fake-cli@'
T="upgrade dnf-пакунок"; run upgrade firefox; rc 0; called '^sudo dnf upgrade --refresh firefox$'; notcalled '^(brew upgrade|go install|npm install)'
T="upgrade flatpak"; run upgrade edge; rc 0; called '^flatpak update --user com.microsoft.Edge'; notcalled 'flatpak update --system'
T="upgrade brew"; run upgrade glab; rc 0; called '^brew upgrade glab$'
T="upgrade appimage"; run upgrade bruno; rc 0; called "^app-manager update $H/Applications/Bruno"; notcalled 'update-all'
T="upgrade composer"; run upgrade laravel/installer; rc 0; called '^composer global update laravel/installer --with-all-dependencies'
T="upgrade cargo"; run upgrade ripgrep; rc 0; called '^cargo install ripgrep'
T="upgrade pip"; run upgrade mytool; rc 0; called 'pip install --user --upgrade mytool'; notcalled 'upgrade somedep|uv tool upgrade'
T="upgrade uv"; run upgrade ruff; rc 0; called '^uv tool upgrade ruff'; notcalled 'pip install|--all'
T="upgrade окрема програма"; run upgrade droid; rc 0; called '^droid update$'; notcalled '^claude update'
T="upgrade sdkman"; run upgrade java; rc 0; called '^sdk upgrade java'; notcalled '^sdk selfupdate'
T="upgrade android"; run upgrade platform-tools; rc 0; called '^sdkmanager platform-tools'; notcalled 'sdkmanager --update'
T="upgrade android вкладений"; run upgrade build-tools/35.0.0; rc 0; called '^sdkmanager build-tools;35.0.0'
T="upgrade fish-плагін"; run upgrade jorgebucaran/fisher; rc 0; called 'fisher update jorgebucaran/fisher'
T="upgrade прошивка"; run upgrade firmware:SSD; rc 0; called '^fwupdmgr update abc123'
T="upgrade кілька"; run upgrade lazygit glab; rc 0; called 'go install .*lazygit'; called '^brew upgrade glab'; notcalled 'go install .*/air'
T="upgrade джерело"; run upgrade go; rc 0; called 'go install .*lazygit'; called 'go install .*/air'; notcalled '^(sudo|brew|npm)'
T="upgrade джерело dnf"; run upgrade dnf; rc 0; called '^sudo dnf upgrade --refresh$'; notcalled '^brew'
T="upgrade джерело + пакунок"; run upgrade dnf firefox; rc 0; called '^sudo dnf upgrade --refresh firefox$'
T="upgrade нема такого"; run upgrade zzznothing; rcfail; notcalled "$MUTATING"
T="upgrade помилка в одній назві — нічого не робить"; run upgrade lazygit zzznothing; rcfail; notcalled "$MUTATING"
T="upgrade усе"; run upgrade; rc 0; called '^sudo dnf upgrade --refresh'; called '^brew upgrade$'; called '^go install .*lazygit'; called '^npm install .*fake-cli@2.0.0'; called '^app-manager --update-all'; called '^droid update$'; notcalled '^fwupdmgr update'
T="upgrade -n"; run upgrade -n lazygit; rc 0; has 'lazygit'; notcalled "$MUTATING"
T="upgrade -y"; run upgrade -y firefox; rc 0; called '^sudo dnf upgrade --refresh -y firefox'
T="list manual: папка зі скриптами — не програма"; run list manual; rc 0; has SomeIDE; hasnt '^ +Scripts '
T="сумісність: --only без команди"; run --only npm -n; rc 0; has fake-cli; hasnt '==> dnf'

T="обірваний конвеєр"; : >"$LOG"
OUT=$({ env -i HOME="$H" PATH="$HOSTPATH" UPM_HOST_ROOT="$R/$HOST" LANG=C.UTF-8 timeout 120 "$SCRIPT" list | head -n1; } 2>&1); hasnt 'panick|Broken pipe|обірван'

T="Ctrl+C під час upgrade"; touch "$H/interrupt-brew"; run upgrade; rm "$H/interrupt-brew"
rc 130; has 'Перервано'; has 'Підсумок'; notcalled '^go install'; notcalled '^npm install'

T="Ctrl+C під час update зупиняє решту запитів"; touch "$H/interrupt-go"; run update go; rm "$H/interrupt-go"
rc 130; [[ $(grep -c '^go list -m' <<<"$CALLS") == 1 ]] && ok || bad "після переривання запити тривають: $(grep -c '^go list -m' <<<"$CALLS")"

# ------------------------------------------------------------ Інші дистрибутиви
# Лише утиліти, без жодного менеджера пакунків цієї системи.
for tool in bash sh env cat awk grep sed basename dirname head tail test timeout mktemp; do
  ln -s "$(command -v "$tool")" "$SYS/$tool"
done

root debian 'ID=debian\n'
stub "$N/debian/dpkg" <<'E'
echo amd64
E
stub "$N/debian/dpkg-query" <<'E'
[[ $1 == -S ]] && exit 1
printf 'installed\thello\t2.10-3\tamd64\ninstalled\tlibdeb\t1.0\tamd64\ninstalled\tlibdeb\t1.0\ti386\n'
E
stub "$N/debian/apt-mark" <<'E'
echo hello
E
stub "$N/debian/apt-get" <<'E'
[[ $1 == -s ]] && printf 'Inst hello [2.10-3] (2.10-4 Debian:12 [amd64])\nConf hello (2.10-4 Debian:12 [amd64])\n'
exit 0
E

root arch 'ID=arch\n'
stub "$N/arch/pacman" <<'E'
case $1 in
  -Qen) echo 'tree 2.2.1-1' ;;
  -Qn) printf 'tree 2.2.1-1\nzlib 1.3-1\n' ;;
  -Qem|-Qm) echo 'downgrade 1.0-1' ;;
  -Qo) exit 1 ;;
  -Qun) echo 'tree 2.2.1-1 -> 2.3.2-1' ;;
esac
E
stub "$N/arch/checkupdates" <<'E'
echo 'tree 2.2.1-1 -> 2.3.2-1'
E
stub "$N/arch/paru" <<'E'
[[ $1 == -Qua ]] && echo 'downgrade 1.0-1 -> 2.0-1'
exit 0
E

for suse in leap:opensuse-leap tumbleweed:opensuse-tumbleweed; do
  name=${suse%:*}
  root "$name" "ID=${suse#*:}\nID_LIKE=\"opensuse suse\"\n"
  stub "$N/$name/rpm" <<'E'
[[ $1 == -qf ]] && exit 1
printf 'less 643-1\nlibsuse 1-1\n'
E
  stub "$N/$name/zypper" <<'E'
case "$*" in
  *userinstalled*) echo 'i+ | Main | less | 643-1 | x86_64' ;;
  *list-updates*) echo 'v  | Main | less | 643-1 | 643-2 | x86_64' ;;
esac
E
done

root alpine 'ID=alpine\n'; mkdir -p "$R/alpine/etc/apk"; echo tree >"$R/alpine/etc/apk/world"
stub "$N/alpine/apk" <<'E'
case "$*" in
  "info -v") printf 'tree-2.1.1-r0\nmusl-1.2.5-r0\n' ;;
  version*) printf 'Installed: Available:\ntree-2.1.1-r0 < 2.1.1-r1\n' ;;
  "info --who-owns"*) exit 1 ;;
esac
E

root centos 'ID="centos"\nID_LIKE="rhel fedora"\n'
stub "$N/centos/rpm" <<'E'
case $1 in
  -qf) exit 1 ;;
  -qa) printf 'bash 4.2-34\nlibyum 1-1\n' ;;
  -q) echo 4.2-34 ;;
esac
E
stub "$N/centos/repoquery" <<'E'
printf 'bash user\nlibyum dep\n'
E
stub "$N/centos/yum" <<'E'
[[ $1 == check-update ]] && { echo 'bash.x86_64   4.2-35   updates'; exit 100; }
exit 0
E

root nixos 'ID=nixos\n'
stub "$N/nixos/nix-env" <<'E'
case "$*" in
  "-q --json") echo '{"0":{"name":"ripgrep-14.0","pname":"ripgrep-nix","version":"14.0"}}' ;;
  *--dry-run) echo "upgrading 'ripgrep-nix-14.0' to 'ripgrep-nix-15.0'" >&2 ;;
esac
E

root atomic 'ID=fedora\n' run/ostree-booted
stub "$N/atomic/rpm" <<'E'
[[ $1 == -qf ]] && exit 1
echo 'ostree-pkg 1-1'
E
stub "$N/atomic/dnf" <<'E'
exit 0
E

# Зміни встановленого рідними менеджерами (оновлення списків репозиторіїв — не зміна).
NATIVE_CHANGE='^(sudo (dnf (upgrade|remove)|yum (update|remove)|(env [^ ]+ )?apt-get .*(upgrade|install|remove) |apt-get (upgrade|install|remove)|zypper (--non-interactive )?(update|dup|remove)|pacman -(S|R)|apk (--interactive )?(upgrade|del)|snap (refresh|remove))|(paru|yay) -S|nix-env --(upgrade$|uninstall))'
# only <шаблон> — з менеджерів дистрибутивів викликано тільки дозволені.
only() {
  local others
  others=$(grep -E '^(sudo )?(dnf|yum|zypper|apt-get|apt-mark|dpkg|dpkg-query|pacman|checkupdates|paru|apk|repoquery|nix-env) ' <<<"$CALLS" | grep -vE "^(sudo )?($1) ")
  [[ -z $others ]] && ok || bad "чужі менеджери: $(head -3 <<<"$others")"
}

host debian
T="debian: list"; run list; rc 0; has '==> apt'; has 'hello +2\.10-3'; hasnt 'libdeb'; hasnt '==> (dnf|yum|zypper|pacman|aur|apk|rpm)'; only 'apt-mark|dpkg|dpkg-query'; notcalled "$NATIVE_CHANGE"
T="debian: list --all, дві архітектури"; run list apt --all; has 'libdeb:i386'
T="debian: update"; run update; rc 0; has '==> apt'; has 'hello +2\.10-3 +2\.10-4'; hasnt '==> (dnf|pacman|zypper|apk)'; called '^sudo apt-get update'; called '^apt-get -s upgrade --with-new-pkgs'; only 'apt-get|apt-mark|dpkg|dpkg-query'; notcalled "$NATIVE_CHANGE"
T="debian: update -n без sudo"; run update -n apt; rc 0; notcalled '^sudo'
T="debian: названого чужого джерела тут немає"; run update dnf; rc 0; has 'тут цього джерела немає'; notcalled '^(sudo )?dnf'
T="debian: upgrade пакунка"; run upgrade apt:hello; rc 0; called '^sudo apt-get install --only-upgrade --no-remove hello$'
T="debian: upgrade -y без термінала не питає"; run upgrade -y deb; rc 0; called '^sudo env DEBIAN_FRONTEND=noninteractive apt-get .*--force-confold upgrade --with-new-pkgs -y$'

host arch
T="arch: list"; run list; rc 0; has '==> pacman'; has '==> aur'; has 'downgrade'; hasnt 'zlib'; hasnt '==> (dnf|apt|zypper|apk|rpm)'; only 'pacman'; notcalled "$NATIVE_CHANGE"
T="arch: update"; run update; rc 0; has 'tree +2\.2\.1-1 +2\.3\.2-1'; has 'downgrade +1\.0-1 +2\.0-1'; called '^checkupdates'; called '^paru -Qua'; only 'pacman|checkupdates|paru'; notcalled '^(sudo )?pacman -Sy'; notcalled "$NATIVE_CHANGE"
T="arch: окремий пакунок pacman не оновлюється"; run upgrade -y pacman:tree; rc 0; has 'цілком'; has 'Пропущено: pacman'; notcalled "$NATIVE_CHANGE"
T="arch: усе джерело pacman"; run upgrade -y pacman; rc 0; called '^sudo pacman -Syu --noconfirm$'
T="arch: без -y і термінала нічого"; run upgrade pacman; rc 0; has 'Пропущено: pacman'; notcalled "$NATIVE_CHANGE"
T="arch: AUR без термінала не збирається"; run upgrade -y aur; rc 0; has 'термінал'; has 'Пропущено: aur'; notcalled '^(paru|yay) -S'

host leap
T="leap: list"; run list; rc 0; has '==> zypper'; has 'less +643-1'; hasnt 'libsuse'; hasnt '==> (dnf|rpm|apt|pacman|apk)'; notcalled "$NATIVE_CHANGE"
T="leap: update"; run update; rc 0; has 'less +643-1 +643-2'; called '^sudo zypper refresh'; only 'zypper'; notcalled "$NATIVE_CHANGE"
T="leap: upgrade"; run upgrade -y zypper; rc 0; called '^sudo zypper --non-interactive update$'
T="leap: rpm означає zypper"; run upgrade -y rpm:less; rc 0; called '^sudo zypper --non-interactive update less$'

host tumbleweed
T="tumbleweed: повне оновлення — dup"; run upgrade -y zypper; rc 0; called '^sudo zypper --non-interactive dup$'

host alpine
T="alpine: list"; run list; rc 0; has '==> apk'; has 'tree +2\.1\.1-r0'; hasnt 'musl'; hasnt '==> (dnf|apt|pacman|zypper|rpm)'; notcalled "$NATIVE_CHANGE"
T="alpine: update"; run update; rc 0; has 'tree +2\.1\.1-r0 +2\.1\.1-r1'; called '^sudo apk update'; only 'apk'; notcalled "$NATIVE_CHANGE"
T="alpine: upgrade пакунка"; run upgrade -y apk:tree; rc 0; called '^sudo apk upgrade tree$'

host centos
T="yum: list"; run list; rc 0; has '==> yum'; has 'bash +4\.2-34'; hasnt 'libyum'; hasnt '==> (dnf|rpm|apt) '; notcalled "$NATIVE_CHANGE"
T="yum: rpm означає yum"; run list rpm; rc 0; has '==> yum'
T="yum: update"; run update; rc 0; has 'bash +4\.2-34 +4\.2-35'; called '^sudo yum check-update -q'; only 'yum|repoquery'; notcalled "$NATIVE_CHANGE"
T="yum: upgrade"; run upgrade -y yum; rc 0; called '^sudo yum update -y$'

host nixos
T="nixos: list"; run list; rc 0; hasnt '==> (nix) '; hasnt '==> (dnf|apt|pacman|zypper|apk|rpm)'
mkdir -p "$H/.nix-profile"; : >"$H/.nix-profile/manifest.nix"
T="nixos: профіль користувача"; run list; rc 0; has '==> nix'; has 'ripgrep-nix +14\.0'; only 'nix-env'
T="nixos: update"; run update nix; rc 0; has 'ripgrep-nix +14\.0 +15\.0'; notcalled "$NATIVE_CHANGE"
T="nixos: upgrade без sudo"; run upgrade nix; rc 0; called '^nix-env --upgrade$'; notcalled '^sudo'
rm -rf "$H/.nix-profile"

host atomic
T="atomic: лише перегляд rpm"; run list; rc 0; has '==> rpm'; has 'ostree-pkg'; hasnt '==> dnf'; notcalled '^(sudo )?dnf'
T="atomic: неперевірене не видається за актуальне"; run update rpm; rc 0; has 'перевірити нема чим: 1'; hasnt 'оновлень не знайдено'
T="atomic: upgrade нічого не міняє"; run upgrade -y rpm; rc 0; has 'лише показує'; has 'Пропущено: rpm'; notcalled "$NATIVE_CHANGE"; notcalled '^(sudo )?dnf'
host fedora

echo "PASS=$PASS FAIL=$FAIL (робоча тека: $W)"
((FAIL == 0))
