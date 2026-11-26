#!/usr/bin/env bash
# Чорно-скринькові тести list/update/upgrade. Усі менеджери пакетів — заглушки,
# HOME — підроблений, тож на справжній системі нічого не змінюється.
# Використання: tests/blackbox.sh <шлях до update-all.sh або бінарника upm>
SCRIPT=$(realpath "${1:?вкажіть програму для перевірки}")
W=$(mktemp -d "${TMPDIR:-/tmp}/upm-test.XXXXXX")
H=$W/home B=$W/stubs LOG=$W/calls.log
mkdir -p "$H" "$B"
PASS=0 FAIL=0

stub() { # stub <шлях> ; тіло зі stdin
  mkdir -p "$(dirname "$1")"
  { echo '#!/bin/bash'; echo "echo \"\$(basename \"\$0\") \$*\" >>'$LOG'"; cat; } >"$1"
  chmod +x "$1"
}

stub "$B/sudo" <<'E'
case $1 in -v|-n) exit 0 ;; esac
[[ $1 == dnf ]] && { shift; exec "$(dirname "$0")/dnf" "$@"; }
exit 0
E
stub "$B/dnf" <<'E'
case "$*" in
  *repoquery*) printf 'bash 5.3-1\nfirefox 140-1\n' ;;
  *check-upgrade*) printf 'Upgrades\nfirefox.x86_64     141-1   updates\nlibdep.x86_64   2-1   updates\n\nObsoleting packages\nnewthing.x86_64 1 updates\n    oldthing.x86_64 1 @System\n'; exit 100 ;;
esac
E
stub "$B/rpm" <<'E'
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
run() { : >"$LOG"; OUT=$(env -i HOME="$H" PATH="$B:/usr/bin:/bin" LANG=C.UTF-8 timeout 120 "$SCRIPT" "$@" </dev/null 2>&1); RC=$?; CALLS=$(cat "$LOG"); }
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

T="list усе"; run list; rc 0
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

T="update усе"; run update; rc 0
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
OUT=$({ env -i HOME="$H" PATH="$B:/usr/bin:/bin" LANG=C.UTF-8 timeout 120 "$SCRIPT" list | head -n1; } 2>&1); hasnt 'panick|Broken pipe|обірван'

T="Ctrl+C під час upgrade"; touch "$H/interrupt-brew"; run upgrade; rm "$H/interrupt-brew"
rc 130; has 'Перервано'; has 'Підсумок'; notcalled '^go install'; notcalled '^npm install'

T="Ctrl+C під час update зупиняє решту запитів"; touch "$H/interrupt-go"; run update go; rm "$H/interrupt-go"
rc 130; [[ $(grep -c '^go list -m' <<<"$CALLS") == 1 ]] && ok || bad "після переривання запити тривають: $(grep -c '^go list -m' <<<"$CALLS")"

echo "PASS=$PASS FAIL=$FAIL (робоча тека: $W)"
((FAIL == 0))
