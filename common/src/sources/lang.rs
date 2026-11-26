//! Пакунки мов програмування: npm, Composer, Go, Rust, Python.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::context::{Cmd, Context, file_name, is_executable, list_dir};
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::version::is_newer;

fn basename(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

/// Виконує дібрані команди видалення; після скасування нові не запускає.
fn run_removals(ctx: &mut Context, commands: Vec<Cmd>) -> Status {
    run_removals_at(
        ctx,
        commands.into_iter().map(|cmd| (None, cmd)).collect(),
    )
}

/// Якщо каталог встановлення не writable (наприклад, `/usr/local`), команда
/// виконується через `sudo` — так само, як оновлення того ж пакунка.
fn run_removals_at(ctx: &mut Context, jobs: Vec<(Option<PathBuf>, Cmd)>) -> Status {
    let total = jobs.len();
    let mut success = true;
    let mut done = 0;
    for (dir, cmd) in jobs {
        if ctx.interrupted() {
            break;
        }
        done += 1;
        success &= match dir {
            Some(dir) => ctx.run_in_dir(&dir, cmd),
            None => ctx.run(&cmd),
        };
    }
    if done == 0 {
        Status::Skipped
    } else if done < total {
        Status::Failed
    } else {
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- Node.js (global)
pub struct Npm;

struct NpmPackage {
    name: String,
    version: String,
    prefix: PathBuf,
}

fn npm_prefixes(ctx: &Context) -> Vec<PathBuf> {
    let configured = ctx
        .read(&Cmd::new(["npm", "config", "get", "prefix"]).quiet())
        .unwrap_or_default();
    let candidates = [
        PathBuf::from("/usr/local"),
        ctx.in_home(".npm-global"),
        ctx.in_home(".local"),
        PathBuf::from(configured.trim()),
    ];
    let mut prefixes: Vec<PathBuf> = Vec::new();
    for prefix in candidates {
        let skip = prefix.as_os_str().is_empty() || prefix == Path::new("/usr");
        if !skip && !prefixes.contains(&prefix) {
            prefixes.push(prefix);
        }
    }
    prefixes
}

fn npm_packages(ctx: &Context) -> Vec<NpmPackage> {
    let mut packages = Vec::new();
    for prefix in npm_prefixes(ctx) {
        let mut dirs = Vec::new();
        for dir in list_dir(&prefix.join("lib/node_modules")) {
            if file_name(&dir).starts_with('@') {
                dirs.extend(list_dir(&dir));
            } else {
                dirs.push(dir);
            }
        }
        for dir in dirs {
            let manifest = dir.join("package.json");
            // Символічне посилання — це `npm link`, тобто локальний проєкт.
            if dir.is_symlink() || !manifest.is_file() || ctx.distro_owned(&manifest) {
                continue;
            }
            let json = std::fs::read_to_string(&manifest)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok());
            if let Some(json) = json
                && let (Some(name), Some(version)) = (json["name"].as_str(), json["version"].as_str())
            {
                packages.push(NpmPackage {
                    name: name.to_string(),
                    version: version.to_string(),
                    prefix: prefix.clone(),
                });
            }
        }
    }
    packages
}

fn npm_latest(ctx: &Context, name: &str) -> Option<String> {
    let result = ctx.capture(&Cmd::new(["npm", "view", name, "version"]).quiet());
    let latest = result.stdout.trim().to_string();
    (result.ok() && !latest.is_empty()).then_some(latest)
}

/// Префікс з інвентаря (`info`) визначає, де саме встановлено пакунок.
/// Разом із командою повертаємо каталог, щоб `run_removals_at` міг підвищити
/// права, коли цей префікс належить root (наприклад, `/usr/local`).
fn npm_uninstall_cmds(items: &[Item], keys: &[String]) -> Vec<(Option<PathBuf>, Cmd)> {
    keys.iter()
        .filter_map(|key| items.iter().find(|item| &item.name == key || &item.key == key))
        .map(|item| {
            let dir = (!item.info.is_empty()).then(|| PathBuf::from(&item.info));
            (
                dir,
                Cmd::new(["npm", "uninstall", "-g", "--prefix"])
                    .args([item.info.as_str(), item.name.as_str()]),
            )
        })
        .collect()
}

/// pnpm і bun мають власні глобальні сховища: команду додаємо лише тоді,
/// коли пакунок справді належить відповідному менеджеру.
fn npm_mirror_uninstalls(home: &Path, names: &[String], pnpm: bool, bun: bool) -> Vec<Cmd> {
    let mut commands = Vec::new();
    let unique: BTreeSet<&String> = names.iter().collect();
    if pnpm {
        let global = home.join(".local/share/pnpm/global");
        let mut stores = vec![global.clone()];
        stores.extend(list_dir(&global));
        for name in &unique {
            if stores.iter().any(|store| store.join("node_modules").join(name).is_dir()) {
                commands.push(Cmd::new(["pnpm", "uninstall", "-g", name.as_str()]));
            }
        }
    }
    if bun {
        let manifest = home.join(".bun/install/global/package.json");
        let owned: BTreeSet<String> = std::fs::read_to_string(manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .and_then(|json| json.get("dependencies").cloned())
            .and_then(|deps| deps.as_object().cloned())
            .map(|deps| deps.keys().cloned().collect())
            .unwrap_or_default();
        for name in &unique {
            if owned.contains(name.as_str()) {
                commands.push(Cmd::new(["bun", "uninstall", "-g", name.as_str()]));
            }
        }
    }
    commands
}

impl Source for Npm {
    fn id(&self) -> &'static str {
        "npm"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("npm")
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        npm_packages(ctx)
            .into_iter()
            .map(|package| {
                let short = basename(&package.name).to_string();
                Item::new(package.name, package.version)
                    .info(package.prefix.display().to_string())
                    .alias(short)
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        for package in npm_packages(ctx) {
            if !pick.has(&package.name) {
                continue;
            }
            let Some(latest) = npm_latest(ctx, &package.name) else {
                ctx.diagnostic(&format!(
                    "{}: не вдалося отримати версію з реєстру npm",
                    package.name
                ));
                query.failed = true;
                query.unchecked.push(package.name);
                continue;
            };
            if is_newer(&package.version, &latest) {
                query.rows.push(
                    Update::new(&package.name, &package.version, &latest)
                        .info(package.prefix.display().to_string()),
                );
            }
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        let mut last_prefix = PathBuf::new();
        for package in npm_packages(ctx) {
            if ctx.interrupted() {
                break;
            }
            let NpmPackage {
                name,
                version,
                prefix,
            } = package;
            if !pick.has(&name) {
                continue;
            }
            if prefix != last_prefix {
                ctx.info(&format!("npm, префікс {}", prefix.display()));
                last_prefix = prefix.clone();
            }
            if ctx.dry {
                ctx.info(&format!("{name} {version} → перевірити й оновити до latest"));
                continue;
            }
            let Some(latest) = npm_latest(ctx, &name) else {
                ctx.warn(&format!("{name}: не вдалося отримати версію з реєстру npm"));
                ctx.failed_commands.push(format!("npm view {name} version"));
                success = false;
                continue;
            };
            if is_newer(&version, &latest) {
                ctx.info(&format!("{name} {version} → {latest}"));
                let install = Cmd::new(["npm", "install", "-g", "--prefix"])
                    .args([prefix.display().to_string(), format!("{name}@{latest}")]);
                success &= ctx.run_in_dir(&prefix.join("lib/node_modules"), install);
            } else {
                ctx.info(&format!("{name} {version} — актуальний"));
            }
        }
        if !pick.targeted() {
            if ctx.have("pnpm") && ctx.in_home(".local/share/pnpm/global").is_dir() {
                success &= ctx.run(&Cmd::new(["pnpm", "update", "-g", "--latest"]));
            }
            if ctx.have("bun") && ctx.in_home(".bun/install/global/package.json").is_file() {
                success &= ctx.run(&Cmd::new(["bun", "update", "-g", "--latest"]));
            }
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let items = self.inventory(ctx, false, false).rows;
        let names: Vec<String> = items
            .iter()
            .filter(|item| pick.has(&item.name))
            .map(|item| item.name.clone())
            .collect();
        let mut commands = npm_uninstall_cmds(&items, pick.keys());
        commands.extend(npm_mirror_uninstalls(
            &ctx.home,
            &names,
            ctx.have("pnpm"),
            ctx.have("bun"),
        ).into_iter().map(|cmd| (None, cmd)));
        run_removals_at(ctx, commands)
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- Composer (global)
pub struct Composer;

/// Список "installed" з `composer global show|outdated`.
fn composer_json(ctx: &Context, action: &str, all: bool) -> Option<Vec<Value>> {
    let mut cmd = Cmd::new(["composer", "global", action, "--format=json"]).quiet();
    if !all {
        cmd = cmd.args(["--direct"]);
    }
    let output = ctx.read(&cmd)?;
    // Порожній глобальний проєкт дає порожній вивід або [].
    let data: Value = serde_json::from_str(&output).unwrap_or_default();
    Some(data["installed"].as_array().cloned().unwrap_or_default())
}

fn composer_remove_cmds(keys: &[String]) -> Vec<Cmd> {
    if keys.is_empty() {
        return Vec::new();
    }
    vec![Cmd::new(["composer", "global", "remove"]).args(keys.iter().cloned())]
}

impl Source for Composer {
    fn id(&self) -> &'static str {
        "composer"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("composer")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(packages) = composer_json(ctx, "show", all) else {
            return Query::failure();
        };
        packages
            .iter()
            .filter_map(|package| {
                let name = package["name"].as_str()?;
                let version = package["version"].as_str().unwrap_or_default();
                Some(Item::new(name, version).alias(basename(name)))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let Some(packages) = composer_json(ctx, "outdated", pick.targeted()) else {
            return Query::failure();
        };
        packages
            .iter()
            .filter_map(|package| {
                let name = package["name"].as_str()?;
                let current = package["version"].as_str().unwrap_or_default();
                let latest = package["latest"].as_str()?;
                (latest != current && pick.has(name)).then(|| Update::new(name, current, latest))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let update = Cmd::new(["composer", "global", "update"])
            .args(pick.keys().to_vec())
            .args(["--with-all-dependencies"]);
        let mut success = ctx.run(&update);
        if !pick.targeted() {
            // Нові мажорні версії composer.json не пропустить — лише показуємо їх.
            success &= ctx.run(&Cmd::new(["composer", "global", "outdated", "--direct"]));
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let commands = composer_remove_cmds(pick.keys());
        run_removals(ctx, commands)
    }
}

// ---------------------------------------------------------------- Go
pub struct Go;

struct GoBinary {
    name: String,
    package: String,
    module: String,
    /// Порожньо, якщо джерело невідоме.
    version: String,
}

impl GoBinary {
    fn local_build(&self) -> bool {
        self.version.is_empty() || self.version == "(devel)"
    }
}

fn go_bindir(ctx: &Context) -> PathBuf {
    let env = |name: &str| {
        ctx.read(&Cmd::new(["go", "env", name]))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let bin = env("GOBIN");
    if !bin.is_empty() {
        return PathBuf::from(bin);
    }
    let gopath = env("GOPATH");
    PathBuf::from(gopath.split(':').next().unwrap_or_default()).join("bin")
}

/// Розбирає вивід `go version -m` для одного бінарника.
fn parse_build_info(name: String, info: &str) -> GoBinary {
    let mut binary = GoBinary {
        name,
        package: String::new(),
        module: String::new(),
        version: String::new(),
    };
    for line in info.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        match fields[..] {
            ["path", package, ..] => binary.package = package.to_string(),
            ["mod", module, version, ..] => {
                binary.module = module.to_string();
                binary.version = version.to_string();
            }
            _ => {}
        }
    }
    binary
}

fn go_binaries(ctx: &Context) -> Vec<GoBinary> {
    list_dir(&go_bindir(ctx))
        .into_iter()
        .filter(|path| is_executable(path))
        .map(|path| {
            let query = Cmd::new(["go", "version", "-m"])
                .args([path.display().to_string()])
                .quiet();
            parse_build_info(file_name(&path), &ctx.read(&query).unwrap_or_default())
        })
        .collect()
}

/// `GOTOOLCHAIN=auto` дозволяє завантажити новіший Go, якщо його вимагає пакунок.
fn go_cmd(args: &[&str]) -> Cmd {
    Cmd::new(["go"])
        .args(args.iter().copied())
        .env("GOTOOLCHAIN", "auto")
}

fn go_latest(ctx: &Context, module: &str) -> Option<String> {
    let target = format!("{module}@latest");
    let output = ctx.read(&go_cmd(&["list", "-m", "-f", "{{.Version}}", &target]))?;
    Some(output.trim().to_string()).filter(|version| !version.is_empty())
}

impl Source for Go {
    fn id(&self) -> &'static str {
        "go"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("go")
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        go_binaries(ctx)
            .into_iter()
            .map(|binary| Item::new(binary.name, binary.version).info(binary.package))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        for binary in go_binaries(ctx) {
            if !pick.has(&binary.name) {
                continue;
            }
            if binary.local_build() {
                ctx.diagnostic(&format!(
                    "{}: зібрано локально, невідоме джерело",
                    binary.name
                ));
                query.unchecked.push(binary.name);
                continue;
            }
            let Some(latest) = go_latest(ctx, &binary.module) else {
                ctx.diagnostic(&format!(
                    "{}: не вдалося дізнатися останню версію",
                    binary.name
                ));
                query.failed = true;
                query.unchecked.push(binary.name);
                continue;
            };
            if is_newer(&binary.version, &latest) {
                query
                    .rows
                    .push(Update::new(&binary.name, &binary.version, &latest));
            }
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if !go_bindir(ctx).is_dir() {
            return Status::Skipped;
        }
        let mut success = true;
        for binary in go_binaries(ctx) {
            if ctx.interrupted() {
                break;
            }
            if !pick.has(&binary.name) {
                continue;
            }
            let GoBinary { name, version, .. } = &binary;
            if binary.local_build() {
                ctx.warn(&format!("{name}: зібрано локально, невідоме джерело — пропускаю"));
                continue;
            }
            let target = format!("{}@latest", binary.package);
            if ctx.dry {
                ctx.info(&format!(
                    "{name} {version} → go install {target}, якщо є новіша версія"
                ));
                continue;
            }
            let Some(latest) = go_latest(ctx, &binary.module) else {
                ctx.warn(&format!("{name}: не вдалося дізнатися останню версію"));
                ctx.failed_commands
                    .push(format!("go list -m {}@latest", binary.module));
                success = false;
                continue;
            };
            if is_newer(version, &latest) {
                ctx.info(&format!("{name} {version} → {latest}"));
                success &= ctx.run(&go_cmd(&["install", &target]));
            } else {
                ctx.info(&format!("{name} {version} — актуальний"));
            }
        }
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- Rust
pub struct Rust;

struct Crate {
    name: String,
    version: String,
    /// Порожньо для crates.io, інакше git-адреса чи локальний шлях.
    origin: String,
}

/// Рядки "назва vX.Y.Z:" або "назва vX.Y.Z (звідки):" з `cargo install --list`.
fn parse_crates(listing: &str) -> Vec<Crate> {
    listing
        .lines()
        .filter_map(|line| {
            let line = line.strip_suffix(':')?;
            let (name, rest) = line.split_once(" v")?;
            if name.contains(' ') {
                return None;
            }
            let (version, origin) = match rest.split_once(" (") {
                Some((version, origin)) => (version, origin.trim_end_matches(')')),
                None => (rest, ""),
            };
            Some(Crate {
                name: name.to_string(),
                version: version.to_string(),
                origin: origin.to_string(),
            })
        })
        .collect()
}

fn cargo_crates(ctx: &Context) -> Vec<Crate> {
    if !ctx.have("cargo") {
        return Vec::new();
    }
    let query = Cmd::new(["cargo", "install", "--list"]).quiet();
    parse_crates(&ctx.read(&query).unwrap_or_default())
}

fn rust_uninstall_cmds(keys: &[String]) -> Vec<Cmd> {
    keys.iter()
        .filter_map(|key| {
            if let Some(toolchain) = key.strip_prefix("toolchain:") {
                (!toolchain.is_empty())
                    .then(|| Cmd::new(["rustup", "toolchain", "uninstall", toolchain]))
            } else {
                (!key.is_empty()).then(|| Cmd::new(["cargo", "uninstall", key.as_str()]))
            }
        })
        .collect()
}

impl Source for Rust {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("cargo") || ctx.have("rustup")
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let mut items = Vec::new();
        if ctx.have("rustup") {
            let listing = ctx
                .read(&Cmd::new(["rustup", "toolchain", "list"]).quiet())
                .unwrap_or_default();
            items.extend(
                listing
                    .lines()
                    .filter_map(|line| line.split_whitespace().next())
                    .filter(|name| *name != "no")
                    .map(|name| {
                        Item::new(name, "")
                            .info("toolchain")
                            .key(format!("toolchain:{name}"))
                    }),
            );
        }
        items.extend(cargo_crates(ctx).into_iter().map(|krate| {
            let origin = if krate.origin.is_empty() {
                "cargo".to_string()
            } else {
                krate.origin
            };
            Item::new(krate.name, krate.version).info(origin)
        }));
        items.into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        if ctx.have("rustup") {
            let check = ctx.capture(&Cmd::new(["rustup", "check"]));
            ctx.log(&check.stdout);
            // Вивід rustup лише показуємо, тож стан toolchain-ів тут невідомий.
            let toolchains = self.inventory(ctx, false, false).rows;
            query.unchecked.extend(
                toolchains
                    .into_iter()
                    .map(|item| item.key)
                    .filter(|key| key.starts_with("toolchain:")),
            );
        }
        for krate in cargo_crates(ctx) {
            if !pick.has(&krate.name) {
                continue;
            }
            if !krate.origin.is_empty() {
                ctx.diagnostic(&format!(
                    "{}: з git/локально — перевірте вручну",
                    krate.name
                ));
                query.unchecked.push(krate.name);
                continue;
            }
            let search = Cmd::new(["cargo", "search", "--limit", "1", krate.name.as_str()]).quiet();
            let prefix = format!("{} = ", krate.name);
            let latest = ctx.read(&search).and_then(|output| {
                let line = output.lines().find(|line| line.starts_with(&prefix))?;
                line.split('"').nth(1).map(str::to_string)
            });
            let Some(latest) = latest else {
                ctx.diagnostic(&format!(
                    "{}: не вдалося дізнатися останню версію",
                    krate.name
                ));
                query.failed = true;
                query.unchecked.push(krate.name);
                continue;
            };
            if is_newer(&krate.version, &latest) {
                query
                    .rows
                    .push(Update::new(&krate.name, &krate.version, &latest));
            }
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        if ctx.have("rustup") {
            if pick.targeted() {
                for toolchain in pick.keys().iter().filter_map(|k| k.strip_prefix("toolchain:")) {
                    success &= ctx.run(&Cmd::new(["rustup", "update", toolchain]));
                }
            } else {
                success &= ctx.run(&Cmd::new(["rustup", "update"]));
            }
        }
        let mut names = Vec::new();
        for krate in cargo_crates(ctx) {
            if !pick.has(&krate.name) {
                continue;
            }
            if krate.origin.is_empty() {
                names.push(krate.name);
            } else {
                ctx.note(format!(
                    "cargo: {} встановлено з git/локально — оновіть вручну",
                    krate.name
                ));
            }
        }
        if names.is_empty() {
            if !pick.targeted() {
                ctx.info("Пакунків із crates.io, установлених через cargo install, немає");
            }
        } else if ctx.have("cargo-install-update") {
            let update = Cmd::new(["cargo", "install-update"]);
            let update = if pick.targeted() {
                update.args(names)
            } else {
                update.args(["--all"])
            };
            success &= ctx.run(&update);
        } else {
            for name in names {
                success &= ctx.run(&Cmd::new(["cargo", "install", name.as_str()]));
            }
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let commands = rust_uninstall_cmds(pick.keys());
        run_removals(ctx, commands)
    }
}

// ---------------------------------------------------------------- Python
pub struct Python;

struct Dist {
    name: String,
    version: String,
    /// Установлено з git чи локальної папки, а не з PyPI.
    local: bool,
}

/// Пакунки в site-packages; без `all` — лише встановлені явно (не залежності).
fn python_dists(dir: &Path, all: bool) -> Vec<Dist> {
    list_dir(dir)
        .into_iter()
        .filter(|path| file_name(path).ends_with(".dist-info"))
        .filter(|path| all || path.join("REQUESTED").is_file())
        .filter_map(|path| {
            let metadata = std::fs::read_to_string(path.join("METADATA")).ok()?;
            let field = |prefix: &str| {
                metadata
                    .lines()
                    .find_map(|line| line.strip_prefix(prefix))
                    .unwrap_or_default()
                    .trim()
                    .to_string()
            };
            Some(Dist {
                name: field("Name: "),
                version: field("Version: "),
                local: path.join("direct_url.json").is_file(),
            })
        })
        .collect()
}

fn python_current(ctx: &Context) -> Option<String> {
    if !ctx.have("python3") {
        return None;
    }
    let script = "import sys; print(\"%d.%d\" % sys.version_info[:2])";
    let output = ctx.read(&Cmd::new(["python3", "-I", "-c", script]))?;
    Some(output.trim().to_string())
}

fn site_packages(ctx: &Context, version: &str) -> PathBuf {
    ctx.in_home(&format!(".local/lib/python{version}/site-packages"))
}

/// Бібліотеки, що лишилися від версії Python, якої вже немає.
fn python_orphans(ctx: &Context, current: &str) -> Vec<(String, PathBuf)> {
    list_dir(&ctx.in_home(".local/lib"))
        .into_iter()
        .filter_map(|dir| {
            let version = file_name(&dir).strip_prefix("python")?.to_string();
            let libdir = dir.join("site-packages");
            let orphan = libdir.is_dir()
                && version != current
                && !ctx.have(&format!("python{version}"));
            orphan.then_some((version, libdir))
        })
        .collect()
}

/// Назви на PyPI не розрізняють регістр і символи "-", "_", ".".
fn normalize(name: &str) -> String {
    name.to_lowercase()
        .split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn uv_tools(ctx: &Context) -> Vec<(String, String)> {
    let listing = ctx
        .read(&Cmd::new(["uv", "tool", "list"]).quiet())
        .unwrap_or_default();
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let name = fields.next()?;
            let version = fields.next()?.strip_prefix('v')?;
            Some((name.to_string(), version.to_string()))
        })
        .collect()
}

/// Рядки "назва v1.0 [latest: 2.0]" з `uv tool list --outdated`.
fn parse_uv_outdated(listing: &str) -> Vec<(&str, &str, &str)> {
    listing
        .lines()
        .filter_map(|line| {
            let (head, rest) = line.split_once(" [latest: ")?;
            let (name, current) = head.split_once(" v")?;
            let latest = rest.split(']').next()?;
            Some((name, current.split_whitespace().next()?, latest))
        })
        .collect()
}

fn joined_names(dists: &[Dist]) -> String {
    dists
        .iter()
        .map(|dist| dist.name.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `orphan:` належить Python, якого вже немає, — таке не видаляємо.
fn python_uninstall_cmds(keys: &[String]) -> Vec<Cmd> {
    let mut commands = Vec::new();
    let unique: BTreeSet<&String> = keys.iter().collect();
    for key in unique {
        let Some((kind, name)) = key.split_once(':') else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        match kind {
            "pip" => {
                let name = normalize(name);
                commands.push(Cmd::new([
                    "python3",
                    "-m",
                    "pip",
                    "uninstall",
                    "-y",
                    name.as_str(),
                ]));
            }
            "uv" => commands.push(Cmd::new(["uv", "tool", "uninstall", name])),
            "pipx" => commands.push(Cmd::new(["pipx", "uninstall", name])),
            _ => {}
        }
    }
    commands
}

impl Source for Python {
    fn id(&self) -> &'static str {
        "python"
    }

    fn available(&self, ctx: &Context) -> bool {
        ["python3", "uv", "pipx"].iter().any(|tool| ctx.have(tool))
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let mut items = Vec::new();
        if let Some(current) = python_current(ctx) {
            for dist in python_dists(&site_packages(ctx, &current), all) {
                let info = if dist.local { "pip, локальний/git" } else { "pip" };
                let key = format!("pip:{}", dist.name);
                items.push(Item::new(dist.name, dist.version).info(info).key(key));
            }
            for (version, libdir) in python_orphans(ctx, &current) {
                for dist in python_dists(&libdir, all) {
                    let key = format!("orphan:{}", dist.name);
                    items.push(
                        Item::new(dist.name, dist.version)
                            .info(format!("не працює: Python {version} вже немає"))
                            .key(key),
                    );
                }
            }
        }
        if ctx.have("uv") {
            for (name, version) in uv_tools(ctx) {
                let key = format!("uv:{name}");
                items.push(Item::new(name, version).info("uv tool").key(key));
            }
        }
        if ctx.have("pipx") {
            let listing = ctx
                .read(&Cmd::new(["pipx", "list", "--short"]).quiet())
                .unwrap_or_default();
            items.extend(listing.lines().filter_map(|line| {
                let (name, version) = line.split_once(' ')?;
                Some(
                    Item::new(name, version)
                        .info("pipx")
                        .key(format!("pipx:{name}")),
                )
            }));
        }
        items.into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        if let Some(current) = python_current(ctx) {
            // Для явно названого пакунка перевіряємо, навіть якщо він залежність.
            let (local, remote): (Vec<Dist>, Vec<Dist>) =
                python_dists(&site_packages(ctx, &current), pick.targeted())
                    .into_iter()
                    .partition(|dist| dist.local);
            query
                .unchecked
                .extend(local.into_iter().map(|dist| format!("pip:{}", dist.name)));
            let wanted: HashMap<String, String> =
                remote
                    .into_iter()
                    .map(|dist| (normalize(&dist.name), dist.name))
                    .collect();
            let asked = !pick.targeted() || pick.keys().iter().any(|k| k.starts_with("pip:"));
            if !wanted.is_empty() && asked {
                let list = Cmd::new(["python3", "-P", "-m", "pip", "list", "--user"])
                    .args(["--outdated", "--format=json"])
                    .quiet();
                let data = ctx
                    .read(&list)
                    .and_then(|json| serde_json::from_str::<Vec<Value>>(&json).ok());
                match data {
                    Some(packages) => {
                        for package in packages {
                            let field = |key: &str| package[key].as_str().unwrap_or_default();
                            let Some(name) = wanted.get(&normalize(field("name"))) else {
                                continue;
                            };
                            let key = format!("pip:{name}");
                            if pick.has(&key) {
                                query.rows.push(
                                    Update::new(name, field("version"), field("latest_version"))
                                        .key(key),
                                );
                            }
                        }
                    }
                    None => {
                        ctx.diagnostic("pip: не вдалося отримати список оновлень");
                        query.failed = true;
                        query
                            .unchecked
                            .extend(wanted.values().map(|name| format!("pip:{name}")));
                    }
                }
            }
            for (_, libdir) in python_orphans(ctx, &current) {
                let orphans = python_dists(&libdir, pick.targeted());
                query
                    .unchecked
                    .extend(orphans.iter().map(|dist| format!("orphan:{}", dist.name)));
            }
            if !pick.targeted() {
                for (version, libdir) in python_orphans(ctx, &current) {
                    let names = joined_names(&python_dists(&libdir, false));
                    if !names.is_empty() {
                        ctx.diagnostic(&format!(
                            "Python {version} уже немає, тому не працюють: {names}"
                        ));
                    }
                }
            }
        }
        if ctx.have("uv") {
            let listing = ctx
                .read(&Cmd::new(["uv", "tool", "list", "--outdated"]).quiet())
                .unwrap_or_default();
            for (name, current, latest) in parse_uv_outdated(&listing) {
                let key = format!("uv:{name}");
                if pick.has(&key) {
                    query.rows.push(Update::new(name, current, latest).key(key));
                }
            }
        }
        if ctx.have("pipx") {
            ctx.diagnostic("pipx не вміє лише перевіряти оновлення");
            let installed = self.inventory(ctx, false, false).rows;
            query.unchecked.extend(
                installed
                    .into_iter()
                    .map(|item| item.key)
                    .filter(|key| key.starts_with("pipx:")),
            );
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut success = true;
        let pip = |name: &str| Cmd::new(["python3", "-P", "-m", "pip", "install", "--user", "--upgrade", name]);
        if pick.targeted() {
            let unique: BTreeSet<&String> = pick.keys().iter().collect();
            for key in unique {
                let Some((kind, name)) = key.split_once(':') else {
                    continue;
                };
                match kind {
                    "pip" => success &= ctx.run(&pip(name)),
                    "uv" => success &= ctx.run(&Cmd::new(["uv", "tool", "upgrade", name])),
                    "pipx" => success &= ctx.run(&Cmd::new(["pipx", "upgrade", name])),
                    _ => ctx.note(format!(
                        "{name} стоїть під Python, якого вже немає. Перевстановіть, напр.: uv tool install {name}"
                    )),
                }
            }
            return Status::from_success(success);
        }
        if ctx.have("uv") {
            success &= ctx.run(&Cmd::new(["uv", "tool", "upgrade", "--all"]));
        }
        if ctx.have("pipx") {
            success &= ctx.run(&Cmd::new(["pipx", "upgrade-all"]));
        }
        if let Some(current) = python_current(ctx) {
            for dist in python_dists(&site_packages(ctx, &current), false) {
                if ctx.interrupted() {
                    break;
                }
                if !dist.local {
                    success &= ctx.run(&pip(&dist.name));
                }
            }
            for (version, libdir) in python_orphans(ctx, &current) {
                let names = joined_names(&python_dists(&libdir, false));
                if !names.is_empty() {
                    ctx.note(format!(
                        "Python {version} уже немає, тому не працюють: {names}. Перевстановіть, напр.: uv tool install <назва>"
                    ));
                }
            }
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        let commands = python_uninstall_cmds(pick.keys());
        run_removals(ctx, commands)
    }
}

#[cfg(test)]
mod tests {
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
    fn npm_uninstall_uses_inventory_prefix_for_selected_names() {
        let items = vec![
            Item::new("typescript", "5.9").info("/usr/local"),
            Item::new("prettier", "3").info("/home/user/.npm-global"),
        ];
        let keys = picked(&["typescript", "нема"]);
        let shown: Vec<String> = npm_uninstall_cmds(&items, &keys)
            .iter()
            .map(|(_, cmd)| cmd.display())
            .collect();
        assert_eq!(shown, vec!["npm uninstall -g --prefix /usr/local typescript"]);
        assert!(Npm.can_remove());
    }

    #[test]
    fn removal_from_a_root_owned_prefix_runs_through_sudo() {
        if crate::running_as_root() {
            return; // root can write anywhere, so no elevation is needed
        }
        let home = temp_home("npm-sudo");
        let locked = home.join("locked");
        std::fs::create_dir_all(&locked).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();

        let (mut ctx, lines) = dry();
        let jobs = vec![(
            Some(locked.clone()),
            Cmd::new(["npm", "uninstall", "-g", "--prefix"])
                .args([locked.display().to_string(), "gemini".to_string()]),
        )];
        assert!(run_removals_at(&mut ctx, jobs) == Status::Ok);
        let shown = commands(&lines);
        assert_eq!(
            shown,
            vec![format!(
                "sudo npm uninstall -g --prefix {} gemini",
                locked.display()
            )]
        );
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).ok();
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn npm_mirrors_are_uninstalled_only_when_owned() {
        let home = temp_home("npm");
        std::fs::create_dir_all(home.join(".local/share/pnpm/global/v10/node_modules/foo"))
            .unwrap();
        std::fs::create_dir_all(home.join(".bun/install/global")).unwrap();
        std::fs::write(
            home.join(".bun/install/global/package.json"),
            r#"{"dependencies":{"bar":"^1.0"}}"#,
        )
        .unwrap();
        let names = picked(&["foo", "bar", "baz"]);
        let shown: Vec<String> = npm_mirror_uninstalls(&home, &names, true, true)
            .iter()
            .map(Cmd::display)
            .collect();
        assert_eq!(shown, vec!["pnpm uninstall -g foo", "bun uninstall -g bar"]);
        assert!(npm_mirror_uninstalls(&home, &names, false, false).is_empty());
        std::fs::remove_dir_all(home).ok();
    }

    #[test]
    fn composer_remove_uses_global_with_selected_names() {
        let keys = picked(&["psr/log", "phpunit/phpunit"]);
        let (mut ctx, lines) = dry();
        assert!(Composer.can_remove());
        assert!(Composer.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        assert_eq!(
            commands(&lines),
            vec!["composer global remove psr/log phpunit/phpunit"]
        );

        let (mut ctx, lines) = dry();
        assert!(Composer.remove(&mut ctx, Pick(None)) == Status::Skipped);
        assert!(lines.lock().unwrap().is_empty());
    }

    #[test]
    fn rust_remove_distinguishes_toolchains_and_crates() {
        let keys = picked(&["toolchain:stable", "ripgrep", "toolchain:1.84.0"]);
        let (mut ctx, lines) = dry();
        assert!(Rust.can_remove());
        assert!(Rust.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        assert_eq!(
            commands(&lines),
            vec![
                "rustup toolchain uninstall stable",
                "cargo uninstall ripgrep",
                "rustup toolchain uninstall 1.84.0",
            ]
        );
    }

    #[test]
    fn python_remove_skips_orphans_and_normalizes_names() {
        let keys = picked(&[
            "pip:Tree_Sitter.C-Sharp",
            "uv:ruff",
            "pipx:black",
            "orphan:old",
            "pip:",
        ]);
        let (mut ctx, lines) = dry();
        assert!(Python.can_remove());
        assert!(Python.remove(&mut ctx, Pick(Some(&keys))) == Status::Ok);
        assert_eq!(
            commands(&lines),
            vec![
                "python3 -m pip uninstall -y tree-sitter-c-sharp",
                "pipx uninstall black",
                "uv tool uninstall ruff",
            ]
        );
    }

    #[test]
    fn crates_distinguish_registry_from_git() {
        let crates = parse_crates(
            "ripgrep v14.0.0:\n    rg\nlocal v0.1.0 (/home/u/local):\n    local\n",
        );
        assert_eq!(crates.len(), 2);
        assert_eq!((crates[0].name.as_str(), crates[0].origin.as_str()), ("ripgrep", ""));
        assert_eq!(crates[1].origin, "/home/u/local");
    }

    #[test]
    fn go_build_info_gives_package_and_module() {
        let binary = parse_build_info(
            "dlv".into(),
            "/x/dlv: go1.26\n\tpath\tex.com/delve/cmd/dlv\n\tmod\tex.com/delve\tv1.2.0\th1:x\n\tdep\ta\tv9\n",
        );
        assert_eq!(binary.package, "ex.com/delve/cmd/dlv");
        assert_eq!(binary.module, "ex.com/delve");
        assert_eq!(binary.version, "v1.2.0");
    }

    #[test]
    fn pypi_names_are_normalized() {
        assert_eq!(normalize("Tree_Sitter.C-Sharp"), "tree-sitter-c-sharp");
    }

    #[test]
    fn uv_outdated_lines_are_parsed() {
        assert_eq!(
            parse_uv_outdated("ruff v0.1.0 [latest: 0.2.0]\n- ruff\n"),
            [("ruff", "0.1.0", "0.2.0")]
        );
    }
}
