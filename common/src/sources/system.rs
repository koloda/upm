//! Системні джерела: DNF, Flatpak, Homebrew, AppImage, прошивки.

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

use crate::context::{Cmd, Context};
use crate::source::{Item, Pick, Query, Source, Status, Update};

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

// ---------------------------------------------------------------- DNF
pub struct Dnf;

/// Рядки "name.arch evr repo" з виводу `dnf check-upgrade`.
fn parse_check_upgrade(listing: &str) -> Vec<(&str, &str)> {
    listing
        .lines()
        .take_while(|line| !line.starts_with("Obsolet"))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [package, version, _repo] = fields[..] else {
                return None;
            };
            let (_, arch) = package.rsplit_once('.')?;
            ["x86_64", "noarch", "i686", "aarch64"]
                .contains(&arch)
                .then_some((package, version))
        })
        .collect()
}

impl Source for Dnf {
    fn id(&self) -> &'static str {
        "dnf"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("dnf") && ctx.have("rpm")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let query = if all {
            Cmd::new(["rpm", "-qa", "--qf", "%{NAME} %{VERSION}-%{RELEASE}\\n"])
        } else {
            Cmd::new(["dnf", "--cacheonly", "--quiet", "repoquery", "--userinstalled"])
                .args(["--qf", "%{name} %{evr}\\n"])
        };
        let Some(listing) = ctx.read(&query) else {
            return Query::failure();
        };
        let unique: BTreeSet<(&str, &str)> = listing
            .lines()
            .filter_map(|line| line.split_once(' '))
            .collect();
        unique
            .into_iter()
            .map(|(name, version)| Item::new(name, version))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let check = if ctx.dry {
            Cmd::new(["dnf", "check-upgrade", "--cacheonly"])
        } else {
            Cmd::new(["sudo", "dnf", "check-upgrade", "--refresh"]).interactive()
        };
        let result = ctx.capture(&check);
        // 100 — "є оновлення".
        if !matches!(result.code, Some(0 | 100)) {
            return Query::failure();
        }
        let mut updates = Vec::new();
        for (package, latest) in parse_check_upgrade(&result.stdout) {
            let name = package.rsplit_once('.').map_or(package, |(name, _)| name);
            if !pick.has(name) {
                continue;
            }
            let installed = Cmd::new(["rpm", "-q", "--qf", "%{EVR}\\n", package]).quiet();
            let current = ctx.read(&installed).unwrap_or_default();
            let shown = package
                .trim_end_matches(".x86_64")
                .trim_end_matches(".noarch");
            let current = current.lines().last().unwrap_or_default();
            updates.push(Update::new(shown, current, latest).key(name));
        }
        updates.into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut cmd = Cmd::new(["sudo", "dnf", "upgrade", "--refresh"]);
        if ctx.yes {
            cmd = cmd.args(["-y"]);
        }
        Status::from_success(ctx.run(&cmd.args(pick.keys().to_vec())))
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- Flatpak
pub struct Flatpak;

const FLATPAK_COLUMNS: &str = "--columns=application,version,name,installation";

impl Source for Flatpak {
    fn id(&self) -> &'static str {
        "flatpak"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("flatpak")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let mut query = Cmd::new(["flatpak", "list", FLATPAK_COLUMNS]);
        if !all {
            query = query.args(["--app"]);
        }
        let Some(listing) = ctx.read(&query) else {
            return Query::failure();
        };
        listing
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split('\t').collect();
                let [app, version, title, installation] = fields[..] else {
                    return None;
                };
                let short = app.rsplit('.').next().unwrap_or(app);
                Some(
                    Item::new(app, version)
                        .info(format!("{title} ({installation})"))
                        .key(format!("{installation} {app}"))
                        .alias(short)
                        .alias(title),
                )
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let installed = ctx
            .read(&Cmd::new(["flatpak", "list", FLATPAK_COLUMNS]))
            .unwrap_or_default();
        let current: HashMap<String, &str> = installed
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split('\t').collect();
                let [app, version, _, installation] = fields[..] else {
                    return None;
                };
                Some((format!("{installation} {app}"), version))
            })
            .collect();

        let mut query: Query<Update> = Query::new();
        for installation in ["user", "system"] {
            let cmd = Cmd::new(["flatpak", "remote-ls", "--updates"])
                .args([format!("--{installation}")])
                .args(["--columns=application,version,branch"]);
            let Some(listing) = ctx.read(&cmd) else {
                query.failed = true;
                continue;
            };
            for line in listing.lines().filter(|line| !line.is_empty()) {
                let mut fields = line.split('\t');
                let app = fields.next().unwrap_or_default();
                let version = fields.next().unwrap_or_default();
                let branch = fields.next().unwrap_or_default();
                let key = format!("{installation} {app}");
                if !pick.has(&key) {
                    continue;
                }
                let old = current.get(&key).copied().unwrap_or_default();
                // Та сама версія в оновленнях означає перезбирання.
                let mut latest = if version.is_empty() || version == old {
                    "нова збірка"
                } else {
                    version
                }
                .to_string();
                if !branch.is_empty() {
                    latest.push_str(&format!(" [{branch}]"));
                }
                query
                    .rows
                    .push(Update::new(app, old, &latest).key(key).info(installation));
            }
        }
        query
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let mut base = Cmd::new(["flatpak", "update"]);
        if ctx.yes {
            base = base.args(["-y", "--noninteractive"]);
        }
        let mut success = true;
        if pick.targeted() {
            for key in pick.keys() {
                if let Some((installation, app)) = key.split_once(' ') {
                    let flag = format!("--{installation}");
                    success &= ctx.run(&base.clone().args([flag.as_str(), app]));
                }
            }
        } else {
            for flag in ["--user", "--system"] {
                success &= ctx.run(&base.clone().args([flag]));
            }
        }
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- Homebrew
pub struct Brew;

fn brew(args: &[&str]) -> Cmd {
    Cmd::new(["brew"])
        .args(args.iter().copied())
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
}

impl Source for Brew {
    fn id(&self) -> &'static str {
        "brew"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("brew")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(formulae) = ctx.read(&brew(&["list", "--formula", "--versions"])) else {
            return Query::failure();
        };
        let requested: Option<BTreeSet<String>> = (!all).then(|| {
            ctx.read(&brew(&["leaves", "--installed-on-request"]))
                .unwrap_or_default()
                .lines()
                // Формули зі сторонніх tap мають назву "власник/tap/формула".
                .map(|line| line.rsplit('/').next().unwrap_or(line).to_string())
                .collect()
        });
        let casks = ctx
            .read(&brew(&["list", "--cask", "--versions"]).quiet())
            .unwrap_or_default();

        let parse = |line: &str, kind: &str| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            (fields.len() >= 2)
                .then(|| Item::new(fields[0], fields[fields.len() - 1]).info(kind))
        };
        formulae
            .lines()
            .filter_map(|line| parse(line, "formula"))
            .filter(|item| requested.as_ref().is_none_or(|set| set.contains(&item.name)))
            .chain(casks.lines().filter_map(|line| parse(line, "cask")))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        if !ctx.dry && !ctx.refresh(&Cmd::new(["brew", "update"])) {
            return Query::failure();
        }
        let json = ctx.read(&brew(&["outdated", "--json=v2"]));
        let Some(data) = json.and_then(|json| serde_json::from_str::<Value>(&json).ok()) else {
            return Query::failure();
        };
        array(&data, "formulae")
            .iter()
            .chain(array(&data, "casks"))
            .filter(|package| pick.has(text(package, "name")))
            .map(|package| {
                let installed = &package["installed_versions"];
                let current = installed
                    .as_array()
                    .and_then(|versions| versions.last())
                    .unwrap_or(installed)
                    .as_str()
                    .unwrap_or_default();
                Update::new(text(package, "name"), current, text(package, "current_version"))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        // Старі версії формул не прибираємо.
        let keep = |cmd: Cmd| {
            cmd.env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
                .env("HOMEBREW_NO_AUTOREMOVE", "1")
        };
        let success = ctx.run(&keep(Cmd::new(["brew", "update"])))
            && ctx.run(&keep(
                Cmd::new(["brew", "upgrade"]).args(pick.keys().to_vec()),
            ));
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- AppImage
pub struct AppImage;

/// Збирає всі об'єкти з "name" на будь-якій глибині реєстру AppManager.
fn collect_apps(value: &Value, items: &mut Vec<Item>) {
    match value {
        Value::Object(map) if map.contains_key("name") => items.push(
            Item::new(text(value, "name"), text(value, "version"))
                .info(text(value, "installed_path")),
        ),
        Value::Object(map) => map.values().for_each(|v| collect_apps(v, items)),
        Value::Array(list) => list.iter().for_each(|v| collect_apps(v, items)),
        _ => {}
    }
}

/// Рядок "Назва: 1.0 -> 2.0".
fn parse_update_line(line: &str) -> Option<Update> {
    let (head, latest) = line.rsplit_once(" -> ")?;
    let (name, current) = head.rsplit_once(": ")?;
    (!current.contains(' ') && !latest.contains(' ')).then(|| Update::new(name, current, latest))
}

const APPIMAGE_REGISTRY: &str = ".local/share/app-manager/installations.json";

impl Source for AppImage {
    fn id(&self) -> &'static str {
        "appimage"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("app-manager") && ctx.in_home(APPIMAGE_REGISTRY).is_file()
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let registry = std::fs::read_to_string(ctx.in_home(APPIMAGE_REGISTRY))
            .ok()
            .and_then(|json| serde_json::from_str::<Value>(&json).ok());
        let Some(registry) = registry else {
            return Query::failure();
        };
        let mut items = Vec::new();
        collect_apps(&registry, &mut items);
        items.into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let Some(listing) = ctx.read(&Cmd::new(["app-manager", "--update-check"])) else {
            return Query::failure();
        };
        listing
            .lines()
            .filter_map(parse_update_line)
            .filter(|update| pick.has(&update.name))
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if !pick.targeted() {
            return Status::from_success(ctx.run(&Cmd::new(["app-manager", "--update-all"])));
        }
        let mut success = true;
        for item in self.inventory(ctx, false, false).rows {
            if pick.has(&item.key) {
                success &= ctx.run(&Cmd::new(["app-manager", "update", item.info.as_str()]));
            }
        }
        Status::from_success(success)
    }
}

// ---------------------------------------------------------------- Прошивки
pub struct Firmware;

impl Source for Firmware {
    fn id(&self) -> &'static str {
        "firmware"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("fwupdmgr")
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let json = ctx.read(&Cmd::new(["fwupdmgr", "get-devices", "--json"]).quiet());
        let Some(data) = json.and_then(|json| serde_json::from_str::<Value>(&json).ok()) else {
            return Query::failure();
        };
        array(&data, "Devices")
            .iter()
            .filter(|device| {
                array(device, "Flags")
                    .iter()
                    .any(|flag| flag.as_str() == Some("updatable"))
            })
            .map(|device| {
                Item::new(text(device, "Name"), text(device, "Version"))
                    .info("прошивка")
                    .key(text(device, "DeviceId"))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        if !ctx.dry {
            // Помилка refresh означає лише, що метадані ще свіжі.
            let result = ctx.capture(&Cmd::new(["fwupdmgr", "refresh"]));
            ctx.log(&result.stdout);
        }
        let result = ctx.capture(&Cmd::new(["fwupdmgr", "get-updates", "--json"]).quiet());
        // Код 2 — "оновлень немає".
        if !matches!(result.code, Some(0 | 2)) {
            ctx.diagnostic(result.stderr.trim());
            return Query::failure();
        }
        if result.stdout.trim().is_empty() {
            return Query::new();
        }
        let Ok(data) = serde_json::from_str::<Value>(&result.stdout) else {
            return Query::failure();
        };
        array(&data, "Devices")
            .iter()
            .filter(|device| pick.has(text(device, "DeviceId")))
            .filter_map(|device| {
                let release = array(device, "Releases").first()?;
                Some(
                    Update::new(
                        text(device, "Name"),
                        text(device, "Version"),
                        text(release, "Version"),
                    )
                    .key(text(device, "DeviceId")),
                )
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        // refresh повертає помилку, коли метадані ще свіжі, — це не збій.
        if !ctx.run(&Cmd::new(["fwupdmgr", "refresh"])) {
            ctx.failed_commands.pop();
        }
        let update = Cmd::new(["fwupdmgr", "update"]);
        if pick.targeted() {
            let mut success = true;
            for id in pick.keys() {
                success &= ctx.run(&update.clone().args([id.as_str()]));
            }
            return Status::from_success(success);
        }
        Status::from_success(ctx.run(&update))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_upgrade_stops_at_obsoletes() {
        let listing = "Upgrades\nfirefox.x86_64  141-1  updates\n\nObsoleting packages\n\
                       newthing.x86_64 1 updates\n";
        assert_eq!(parse_check_upgrade(listing), [("firefox.x86_64", "141-1")]);
    }

    #[test]
    fn appimage_update_line_allows_spaces_in_name() {
        let update = parse_update_line("Desktop Plus: 3.6.4 -> 3.6.12").unwrap();
        assert_eq!(update.name, "Desktop Plus");
        assert_eq!((update.current.as_str(), update.latest.as_str()), ("3.6.4", "3.6.12"));
        assert!(parse_update_line("No updates available").is_none());
    }
}
