//! Окремі програми, SDK, плагіни та те, що оновлюється вручну.

use std::path::PathBuf;

use crate::context::{Cmd, Context, file_name, is_executable, list_dir};
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::version::is_newer;

/// Виконує дібрані команди видалення; після скасування нові не запускає.
fn run_removals(ctx: &mut Context, commands: Vec<Cmd>) -> Status {
    let total = commands.len();
    let mut success = true;
    let mut done = 0;
    for cmd in &commands {
        if ctx.interrupted() {
            break;
        }
        done += 1;
        success &= ctx.run(cmd);
    }
    if done == 0 {
        Status::Skipped
    } else if done < total {
        Status::Failed
    } else {
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- Програми з власним оновлювачем
pub struct Tools;

struct Tool {
    name: &'static str,
    path: PathBuf,
}

/// Наявні й не керовані через RPM.
fn present_tools(ctx: &Context) -> Vec<Tool> {
    let local = [
        "claude", "cursor-agent", "droid", "kiro-cli", "omp", "agy", "unity", "qwen", "muse",
    ]
    .map(|name| (name, ctx.in_home(".local/bin").join(name)));
    let elsewhere = [
        ("opencode", ctx.in_home(".opencode/bin/opencode")),
        ("mimo", ctx.in_home(".mimocode/bin/mimo")),
        ("delta", ctx.in_home(".local/delta.app/bin/delta")),
    ];
    let shared = ["ngrok", "phive", "pie"]
        .map(|name| (name, PathBuf::from("/usr/local/bin").join(name)));
    local
        .into_iter()
        .chain(elsewhere)
        .chain(shared)
        .filter(|(_, path)| is_executable(path) && !ctx.rpm_owned(path))
        .map(|(name, path)| Tool { name, path })
        .collect()
}

/// Перший фрагмент, схожий на версію: "1.2", "1.2.3", "2026.11.22-e9042b2".
fn find_version(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let digits = |mut at: usize| {
        while bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at += 1;
        }
        at
    };
    for start in 0..bytes.len() {
        let mut end = digits(start);
        if end == start || bytes.get(end) != Some(&b'.') || digits(end + 1) == end + 1 {
            continue;
        }
        end = digits(end + 1);
        // Далі — будь-яка кількість груп виду ".3", "-beta", "+build".
        while bytes.get(end).is_some_and(|c| b"-.+".contains(c))
            && bytes.get(end + 1).is_some_and(u8::is_ascii_alphanumeric)
        {
            end += 1;
            while bytes.get(end).is_some_and(u8::is_ascii_alphanumeric) {
                end += 1;
            }
        }
        return Some(&text[start..end]);
    }
    None
}

fn tool_version(ctx: &Context, tool: &Tool) -> String {
    let version = if tool.name == "muse" {
        // --version у muse запускає самооновлення, тому читаємо файл.
        let file = tool.path.with_file_name(".muse-version");
        std::fs::read_to_string(file)
            .ok()
            .map(|text| text.trim().to_string())
    } else {
        let query = Cmd::new(["timeout", "20"])
            .args([tool.path.display().to_string()])
            .args(["--version"])
            .quiet();
        find_version(&ctx.capture(&query).stdout).map(str::to_string)
    };
    version
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "не запускається".to_string())
}

/// Нова версія з виводу `<програма> update --check`.
fn parse_available(output: &str) -> Option<&str> {
    output.lines().find_map(|line| {
        let after = ["Update available:", "update available:", "New version available:"]
            .iter()
            .find_map(|marker| line.split_once(marker).map(|(_, rest)| rest))
            .or_else(|| {
                line.strip_prefix("Delta ")
                    .filter(|rest| rest.contains(" is available"))
            })?;
        let word = after.split_whitespace().next()?;
        let word = word.strip_prefix('v').unwrap_or(word);
        word.starts_with(|c: char| c.is_ascii_digit()).then_some(word)
    })
}

fn upgrade_tool(ctx: &mut Context, tool: &Tool) -> bool {
    let Tool { name, path } = tool;
    let mut args = vec!["update"];
    match *name {
        "kiro-cli" if ctx.yes => args.push("--non-interactive"),
        "unity" => {
            args = vec!["self-update"];
            if ctx.yes {
                args.push("--yes");
            }
        }
        "opencode" | "mimo" => args = vec!["upgrade"],
        "phive" => args = vec!["selfupdate"],
        "pie" => args = vec!["self-update"],
        _ => {}
    }
    ctx.info(name);
    let program = path.display().to_string();
    match *name {
        // Команди update немає: оновлюється лаунчером під час запуску.
        "muse" => ctx.run(
            &Cmd::new(["env", "MUSE_SYNC_UPDATE=1", "timeout", "--foreground", "900"])
                .args([program.as_str(), "--version"]),
        ),
        "ngrok" | "phive" | "pie" => {
            let works = |ctx: &Context| {
                ctx.capture(&Cmd::new([program.as_str(), "--version"]).quiet())
                    .ok()
            };
            if *name == "pie" && !works(ctx) {
                ctx.note(
                    "pie не запускається з поточною версією PHP — оновіть його вручну \
                     (новий pie.phar з github.com/php/pie)"
                        .to_string(),
                );
                return true;
            }
            let dir = path.parent().map(PathBuf::from).unwrap_or_default();
            ctx.run_in_dir(&dir, Cmd::new([program.as_str()]).args(args))
        }
        _ => ctx.run(
            &Cmd::new(["timeout", "--foreground", "900", program.as_str()])
                .args(args)
                .interactive(),
        ),
    }
}

impl Source for Tools {
    fn id(&self) -> &'static str {
        "tools"
    }

    fn available(&self, ctx: &Context) -> bool {
        !present_tools(ctx).is_empty()
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, versions: bool) -> Query<Item> {
        present_tools(ctx)
            .iter()
            .map(|tool| {
                let version = if versions {
                    tool_version(ctx, tool)
                } else {
                    String::new()
                };
                Item::new(tool.name, version).info(tool.path.display().to_string())
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        let mut unsupported = Vec::new();
        for tool in present_tools(ctx) {
            if !pick.has(tool.name) {
                continue;
            }
            let check = |args: &[&str]| {
                let cmd = Cmd::new(["timeout", "60"])
                    .args([tool.path.display().to_string()])
                    .args(args.iter().copied())
                    .quiet();
                ctx.capture(&cmd)
            };
            let (current, latest) = match tool.name {
                "droid" | "omp" | "delta" => {
                    let result = check(&["update", "--check"]);
                    if !result.ok() {
                        ctx.diagnostic(&format!("{}: перевірка не вдалася", tool.name));
                        query.unchecked.push(tool.name.to_string());
                    }
                    let text = format!("{}\n{}", result.stdout, result.stderr);
                    (None, parse_available(&text).map(str::to_string))
                }
                "unity" => {
                    let result = check(&["self-update", "--check"]);
                    if !result.ok() {
                        ctx.diagnostic("unity: перевірка не вдалася");
                        query.unchecked.push(tool.name.to_string());
                    }
                    let pair = result.stdout.lines().find_map(|line| {
                        let mut words = line.split_whitespace();
                        let (a, b) = (words.next()?, words.next()?);
                        let version = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
                        (version(a) && version(b)).then(|| (a.to_string(), b.to_string()))
                    });
                    match pair {
                        Some((current, latest)) => (Some(current), Some(latest)),
                        None => (None, None),
                    }
                }
                other => {
                    unsupported.push(other);
                    continue;
                }
            };
            let Some(latest) = latest else { continue };
            let current = current.unwrap_or_else(|| tool_version(ctx, &tool));
            if is_newer(&current, &latest) {
                query.rows.push(Update::new(tool.name, &current, &latest));
            }
        }
        if !unsupported.is_empty() {
            ctx.diagnostic(&format!(
                "не вміють лише перевіряти (оновляться під час upgrade): {}",
                unsupported.join(" ")
            ));
        }
        query
            .unchecked
            .extend(unsupported.into_iter().map(str::to_string));
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        for tool in present_tools(ctx) {
            if ctx.interrupted() {
                break;
            }
            if pick.has(tool.name) {
                success &= upgrade_tool(ctx, &tool);
            }
        }
        Status::from_success(success)
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- SDKMAN (Java тощо)
pub struct Sdkman;

const SDKMAN_INIT: &str = ".sdkman/bin/sdkman-init.sh";

/// `sdk` — функція оболонки, тому спершу підключаємо її init-скрипт.
fn sdk(ctx: &Context, args: &[&str]) -> Cmd {
    Cmd::new(["bash", "-c", "source \"$1\"; shift; sdk \"$@\"", "_"])
        .args([ctx.in_home(SDKMAN_INIT).display().to_string()])
        .args(args.iter().copied())
        .interactive()
}

/// Версію кандидата беремо з посилання `current` у каталозі кандидатів.
fn sdk_currents(ctx: &Context) -> Vec<(String, String)> {
    list_dir(&ctx.in_home(".sdkman/candidates"))
        .into_iter()
        .filter(|dir| dir.is_dir())
        .filter_map(|dir| {
            let version = std::fs::read_link(dir.join("current"))
                .ok()
                .map(|target| file_name(&target))?;
            Some((file_name(&dir), version))
        })
        .collect()
}

/// Синтаксис `sdk uninstall <candidate> <version>` підтверджено `sdk help`
/// на цій машині; без встановленої версії команду не запускаємо.
fn sdk_uninstall_cmds(ctx: &Context, keys: &[String]) -> Vec<Cmd> {
    let currents = sdk_currents(ctx);
    keys.iter()
        .filter_map(|key| currents.iter().find(|(name, _)| name == key))
        .map(|(name, version)| sdk(ctx, &["uninstall", name, version]))
        .collect()
}

impl Source for Sdkman {
    fn id(&self) -> &'static str {
        "sdkman"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.in_home(SDKMAN_INIT)
            .metadata()
            .is_ok_and(|meta| meta.len() > 0)
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        list_dir(&ctx.in_home(".sdkman/candidates"))
            .into_iter()
            .filter_map(|dir| {
                let versions: Vec<String> = list_dir(&dir)
                    .iter()
                    .filter(|path| path.is_dir())
                    .map(|path| file_name(path))
                    .filter(|name| name != "current")
                    .collect();
                if versions.is_empty() {
                    return None;
                }
                let current = std::fs::read_link(dir.join("current"))
                    .map(|target| file_name(&target))
                    .unwrap_or_default();
                let info = format!("встановлені: {}", versions.join(" "));
                Some(Item::new(file_name(&dir), current).info(info))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        if !ctx.dry {
            let update = sdk(ctx, &["update"]);
            query.failed = !ctx.refresh(&update);
        }
        let api = Cmd::new([
            "bash",
            "-c",
            "source \"$1\" >/dev/null 2>&1; printf %s \"$SDKMAN_CANDIDATES_API\"",
            "_",
        ])
        .args([ctx.in_home(SDKMAN_INIT).display().to_string()]);
        let api = ctx.read(&api).unwrap_or_default();
        for item in self.inventory(ctx, false, true).rows {
            if !pick.has(&item.key) {
                continue;
            }
            // Не `sdk upgrade`: без відповіді на запитання він одразу встановлює.
            let url = format!("{api}/candidates/default/{}", item.name);
            let latest = ctx
                .read(&Cmd::new(["curl", "-fsSL", "--max-time", "20", url.as_str()]))
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty());
            let Some(latest) = latest else {
                ctx.diagnostic(&format!(
                    "{}: не вдалося дізнатися останню версію",
                    item.name
                ));
                query.failed = true;
                query.unchecked.push(item.key);
                continue;
            };
            let installed = ctx
                .in_home(".sdkman/candidates")
                .join(&item.name)
                .join(&latest);
            if !installed.is_dir() {
                query
                    .rows
                    .push(Update::new(&item.name, &item.version, &latest));
            }
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        if pick.targeted() {
            for candidate in pick.keys() {
                let upgrade = sdk(ctx, &["upgrade", candidate]);
                success &= ctx.run(&upgrade);
            }
            return Status::from_success(success);
        }
        // `upgrade` лишається інтерактивним навіть з -y: може змінити мажорну версію Java.
        for step in ["selfupdate", "update", "upgrade"] {
            if step == "upgrade" && !ctx.interactive() {
                ctx.note("sdk upgrade потребує термінала — пропущено".to_string());
                continue;
            }
            let command = sdk(ctx, &[step]);
            success &= ctx.run(&command);
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let commands = sdk_uninstall_cmds(ctx, pick.keys());
        run_removals(ctx, commands)
    }
}

// ---------------------------------------------------------------- Android SDK
pub struct Android;

fn sdkmanager(ctx: &Context) -> PathBuf {
    std::env::var_os("ANDROID_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.in_home("Android/Sdk"))
        .join("cmdline-tools/latest/bin/sdkmanager")
}

struct SdkPackage {
    id: String,
    version: String,
    latest: Option<String>,
}

/// Секція "Installed packages:" із виводу sdkmanager.
fn parse_sdk_packages(listing: &str) -> Vec<SdkPackage> {
    let mut packages = Vec::new();
    let mut installed = false;
    for line in listing.split(['\n', '\r']) {
        if line.starts_with("Installed packages:") {
            installed = true;
            continue;
        }
        if line.starts_with("Available") || line.starts_with("Obsolete") {
            installed = false;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        if !installed || words.len() < 2 || !words[0].starts_with(|c: char| c.is_ascii_lowercase()) {
            continue;
        }
        // Старий формат: "id | версія | опис"; новий: "id версія [-> нова] опис".
        let version = if words[1] == "|" {
            words.get(2).copied().unwrap_or_default()
        } else {
            words[1]
        };
        let latest = (words.get(2) == Some(&"->"))
            .then(|| words.get(3).map(|v| v.to_string()))
            .flatten();
        packages.push(SdkPackage {
            id: words[0].to_string(),
            version: version.to_string(),
            latest,
        });
    }
    packages
}

fn sdk_packages(ctx: &Context, mode: &str) -> Option<Vec<SdkPackage>> {
    let query = Cmd::new([sdkmanager(ctx).display().to_string()])
        .args([mode])
        .quiet();
    ctx.read(&query).map(|text| parse_sdk_packages(&text))
}

/// `sdkmanager --uninstall` підтверджено `sdkmanager --help`; ключ інвентаря
/// уже є шляхом встановленого пакета.
fn android_uninstall_cmd(manager: &std::path::Path, key: &str) -> Cmd {
    Cmd::new([manager.display().to_string()])
        .args(["--uninstall", key])
        .interactive()
}

impl Source for Android {
    fn id(&self) -> &'static str {
        "android"
    }

    fn available(&self, ctx: &Context) -> bool {
        is_executable(&sdkmanager(ctx))
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let Some(packages) = sdk_packages(ctx, "--list_installed") else {
            return Query::failure();
        };
        packages
            .into_iter()
            .map(|package| Item::new(package.id, package.version))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let Some(packages) = sdk_packages(ctx, "--list") else {
            return Query::failure();
        };
        packages
            .into_iter()
            .filter(|package| pick.has(&package.id))
            .filter_map(|package| {
                let latest = package.latest?;
                Some(Update::new(&package.id, &package.version, &latest))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let base = Cmd::new([sdkmanager(ctx).display().to_string()]).interactive();
        let cmd = if pick.targeted() {
            base.args(pick.keys().iter().map(|id| id.replace('/', ";")))
        } else {
            base.args(["--update"])
        };
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let manager = sdkmanager(ctx);
        let commands = pick
            .keys()
            .iter()
            .map(|key| android_uninstall_cmd(&manager, key))
            .collect();
        run_removals(ctx, commands)
    }
}

// ---------------------------------------------------------------- Плагіни fish і розширення gh
pub struct Shell;

fn has_fisher(ctx: &Context) -> bool {
    ctx.in_home(".config/fish/functions/fisher.fish").is_file()
}

fn has_gh_extensions(ctx: &Context) -> bool {
    ctx.have("gh") && !list_dir(&ctx.in_home(".local/share/gh/extensions")).is_empty()
}

/// Назва потрапляє в командний рядок fish, тож дозволяємо лише безпечні символи.
fn shell_uninstall_cmd(key: &str) -> Option<Cmd> {
    let (kind, name) = key.split_once(':')?;
    let safe = name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._@/~-".contains(c));
    if name.is_empty() || !safe {
        return None;
    }
    match kind {
        "fisher" => {
            let script = format!("fisher remove {name}");
            Some(Cmd::new(["fish", "-c", script.as_str()]))
        }
        "gh" => {
            let short = name.rsplit('/').next().unwrap_or(name);
            Some(Cmd::new(["gh", "extension", "remove", short]))
        }
        _ => None,
    }
}

impl Source for Shell {
    fn id(&self) -> &'static str {
        "shell"
    }

    fn available(&self, ctx: &Context) -> bool {
        has_fisher(ctx) || has_gh_extensions(ctx)
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let mut items = Vec::new();
        let basename = |name: &str| name.rsplit('/').next().unwrap_or(name).to_string();
        if has_fisher(ctx) {
            let plugins = std::fs::read_to_string(ctx.in_home(".config/fish/fish_plugins"))
                .unwrap_or_default();
            items.extend(
                plugins
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .map(|plugin| {
                        Item::new(plugin, "")
                            .info("плагін fish (fisher)")
                            .key(format!("fisher:{plugin}"))
                            .alias(basename(plugin))
                    }),
            );
        }
        if has_gh_extensions(ctx) {
            let listing = ctx
                .read(&Cmd::new(["gh", "extension", "list"]).quiet())
                .unwrap_or_default();
            items.extend(listing.lines().filter_map(|line| {
                let fields: Vec<&str> = line.split('\t').collect();
                let repo = fields.get(1)?;
                let version = fields.get(2).copied().unwrap_or_default();
                Some(
                    Item::new(*repo, version)
                        .info("розширення gh")
                        .key(format!("gh:{repo}"))
                        .alias(basename(repo)),
                )
            }));
        }
        items.into()
    }

    fn outdated(&self, ctx: &mut Context, _pick: Pick) -> Query<Update> {
        // Жодне з цих джерел не повертає список, який можна показати.
        let unchecked: Vec<String> = self
            .inventory(ctx, false, false)
            .rows
            .into_iter()
            .map(|item| item.key)
            .collect();
        if has_fisher(ctx) {
            ctx.diagnostic("fisher не вміє лише перевіряти оновлення плагінів");
        }
        if has_gh_extensions(ctx) {
            let check = Cmd::new(["gh", "extension", "upgrade", "--all", "--dry-run"]);
            let result = ctx.capture(&check);
            ctx.log(&result.stdout);
            if !result.ok() {
                return Query::failure();
            }
        }
        Query {
            unchecked,
            ..Query::new()
        }
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        if pick.targeted() {
            for key in pick.keys() {
                let Some((kind, name)) = key.split_once(':') else {
                    continue;
                };
                // Назва потрапляє в командний рядок fish, тож дозволяємо лише безпечні символи.
                let safe = name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "._@/~-".contains(c));
                if !safe {
                    ctx.note(format!("дивна назва плагіна, пропущено: {name}"));
                    continue;
                }
                success &= if kind == "fisher" {
                    let script = format!("fisher update {name}");
                    ctx.run(&Cmd::new(["fish", "-c", script.as_str()]))
                } else {
                    let short = name.rsplit('/').next().unwrap_or(name);
                    ctx.run(&Cmd::new(["gh", "extension", "upgrade", short]))
                };
            }
            return Status::from_success(success);
        }
        if ctx.have("fish") && has_fisher(ctx) {
            success &= ctx.run(&Cmd::new(["fish", "-c", "fisher update"]));
        }
        if has_gh_extensions(ctx) {
            success &= ctx.run(&Cmd::new(["gh", "extension", "upgrade", "--all"]));
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut commands = Vec::new();
        for key in pick.keys() {
            if let Some(cmd) = shell_uninstall_cmd(key) {
                commands.push(cmd);
            } else {
                ctx.note(format!("не можу безпечно видалити: {key}"));
            }
        }
        run_removals(ctx, commands)
    }
}

// ---------------------------------------------------------------- Оновлюється вручну
pub struct Manual;

/// Папка в ~/Apps є програмою, якщо це не папка зі скриптами і не місце, де лежить upm.
fn is_application(dir: &std::path::Path) -> bool {
    let own = std::env::current_exe().ok();
    if own
        .zip(dir.canonicalize().ok())
        .is_some_and(|(exe, dir)| exe.starts_with(dir))
    {
        return false;
    }
    let files: Vec<PathBuf> = list_dir(dir).into_iter().filter(|p| p.is_file()).collect();
    let only_scripts = !files.is_empty()
        && files
            .iter()
            .all(|path| path.extension().is_some_and(|ext| ext == "sh"));
    !only_scripts
}

impl Source for Manual {
    fn id(&self) -> &'static str {
        "manual"
    }

    fn available(&self, _ctx: &Context) -> bool {
        true
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let mut items: Vec<Item> = list_dir(&ctx.in_home("Apps"))
            .iter()
            .filter(|path| path.is_dir() && is_application(path))
            .map(|path| Item::new(file_name(path), "").info("~/Apps, оновлюється з самої програми"))
            .collect();
        if is_executable(&ctx.in_home(".local/zed.app/bin/zed")) {
            items.push(Item::new("zed", "").info("оновлює себе сам під час роботи"));
        }
        items.extend(
            list_dir(&ctx.in_home(".local/share/JetBrains/Toolbox/apps"))
                .iter()
                .filter(|path| path.is_dir())
                .map(|path| Item::new(file_name(path), "").info("через JetBrains Toolbox")),
        );
        items.into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        if pick.targeted() {
            ctx.log(&format!(
                "  перевіряється з самої програми: {}\n",
                pick.keys().join(" ")
            ));
        }
        Query::new()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if !pick.targeted() {
            return Status::Skipped;
        }
        ctx.note(format!(
            "оновлюється вручну або з самої програми: {}",
            pick.keys().join(" ")
        ));
        Status::Ok
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::report::Reporter;

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<String>>>);

    impl Reporter for Recorder {
        fn info(&self, _: &str) {}
        fn warn(&self, _: &str) {}
        fn diagnostic(&self, _: &str) {}
        fn log(&self, _: &str) {}
        fn command(&self, line: &str, side: bool) {
            if !side {
                self.0.lock().unwrap().push(line.to_string());
            }
        }
        fn failure(&self, _: &str) {}
        fn captures_output(&self) -> bool {
            true
        }
    }

    /// Команди в режимі --dry-run лише записуються, а не виконуються.
    fn dry() -> (Context, Arc<Mutex<Vec<String>>>) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let ctx = Context::new("/home/user".into(), true, true, Box::new(Recorder(lines.clone())));
        (ctx, lines)
    }

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    fn commands(lines: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        lines.lock().unwrap().clone()
    }

    fn temp_home(tag: &str) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "upm-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn sdkman_uninstall_uses_current_version_from_symlink() {
        let home = temp_home("sdkman");
        let candidate = home.join(".sdkman/candidates/java");
        std::fs::create_dir_all(&candidate).unwrap();
        symlink("21.0.2", candidate.join("current")).unwrap();
        let (mut ctx, lines) = dry();
        ctx.home = home.clone();
        let keys = picked(&["java", "gradle"]);
        assert!(Sdkman.can_remove());
        assert!(Sdkman.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        assert_eq!(
            commands(&lines),
            vec![format!(
                "bash -c source \"$1\"; shift; sdk \"$@\" _ {} uninstall java 21.0.2",
                ctx.in_home(SDKMAN_INIT).display()
            )]
        );
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn android_uninstall_uses_installed_package_ids() {
        let keys = picked(&["platform-tools", "build-tools/35.0.0"]);
        let (mut ctx, lines) = dry();
        assert!(Android.can_remove());
        assert!(Android.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        let manager = sdkmanager(&ctx).display().to_string();
        assert_eq!(
            commands(&lines),
            vec![
                format!("{manager} --uninstall platform-tools"),
                format!("{manager} --uninstall build-tools/35.0.0"),
            ]
        );
    }

    #[test]
    fn shell_uninstall_commands_use_fisher_and_gh_extensions() {
        let keys = picked(&[
            "fisher:jorgebutler/fisher",
            "gh:cli/cli",
            "fisher:bad;name",
            "інше:x",
        ]);
        let (mut ctx, lines) = dry();
        assert!(Shell.can_remove());
        assert!(Shell.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        assert_eq!(
            commands(&lines),
            vec![
                "fish -c fisher remove jorgebutler/fisher",
                "gh extension remove cli",
            ]
        );
        assert!(shell_uninstall_cmd("fisher:bad;name").is_none());
    }

    #[test]
    fn non_removable_sources_never_run_a_command() {
        let keys = picked(&["anything"]);
        let (mut ctx, lines) = dry();
        let mut seen = 0;
        for source in crate::sources::all() {
            if ["go", "tools", "firmware", "manual"].contains(&source.id()) {
                seen += 1;
                assert!(!source.can_remove());
                assert!(source.remove(&mut ctx, Pick(Some(&keys))) == Status::Skipped);
            }
        }
        assert_eq!(seen, 4);
        assert!(lines.lock().unwrap().is_empty());
    }

    #[test]
    fn versions_are_found_in_tool_output() {
        assert_eq!(find_version("2.1.332 (Claude Code)"), Some("2.1.332"));
        assert_eq!(find_version("omp/19.1.1"), Some("19.1.1"));
        assert_eq!(find_version("2026.11.22-e9042b2\n"), Some("2026.11.22-e9042b2"));
        assert_eq!(find_version("ngrok version 3.39.6"), Some("3.39.6"));
        assert_eq!(find_version("no digits. 7 only"), None);
    }

    #[test]
    fn available_versions_are_parsed() {
        assert_eq!(parse_available("! Update available: 1.21.1"), Some("1.21.1"));
        assert_eq!(parse_available("New version available: 19.2.1"), Some("19.2.1"));
        assert_eq!(
            parse_available("Delta 0.29.1 is available (installed: 0.27.2)."),
            Some("0.29.1")
        );
        assert_eq!(parse_available("Already up to date"), None);
    }

    #[test]
    fn sdk_listing_keeps_only_installed() {
        let packages = parse_sdk_packages(
            "Installed packages:\n  emulator   37.1  ->  37.3  Emulator\n  cmake/3  3.22  CMake\n\n\
             Available packages:\n  future/99  99  Future\n",
        );
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].latest.as_deref(), Some("37.3"));
        assert_eq!(packages[1].latest, None);
    }
}
