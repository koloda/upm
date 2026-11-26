//! Менеджери, спільні для різних дистрибутивів: Snap і Nix.

use std::collections::HashMap;

use serde_json::Value;

use crate::context::{Cmd, Context};
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::{query, run_removals};

// ---------------------------------------------------------------- Snap
pub struct Snap;

/// Базовий snap чи середовище виконання, а не програма. `snap list` позначає
/// лише бази та snapd, тож середовища впізнаються за назвою.
fn is_runtime(name: &str, notes: &str) -> bool {
    notes
        .split(',')
        .any(|note| ["base", "core", "snapd"].contains(&note))
        || ["bare", "gtk-common-themes"].contains(&name)
        || ["gnome-4", "gnome-3-", "kf5-", "kf6-", "mesa-"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Стовпці таблиці snap без заголовка.
fn snap_table(listing: &str) -> impl Iterator<Item = Vec<&str>> {
    listing
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|columns| columns.len() >= 5)
}

impl Source for Snap {
    fn id(&self) -> &'static str {
        "snap"
    }

    fn available(&self, ctx: &Context) -> bool {
        // Сокет є лише там, де працює демон snapd.
        ctx.have("snap") && ctx.platform.path("run/snapd.socket").exists()
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(listing) = ctx.read(&query(["snap", "list"])) else {
            return Query::failure();
        };
        snap_table(&listing)
            .filter(|columns| all || !is_runtime(columns[0], columns[columns.len() - 1]))
            .map(|columns| Item::new(columns[0], columns[1]))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let Some(available) = ctx.read(&query(["snap", "refresh", "--list"])) else {
            return Query::failure();
        };
        let installed = ctx.read(&query(["snap", "list"])).unwrap_or_default();
        let current: HashMap<&str, &str> = snap_table(&installed)
            .map(|columns| (columns[0], columns[1]))
            .collect();
        snap_table(&available)
            .filter(|columns| pick.has(columns[0]))
            .map(|columns| {
                let old = current.get(columns[0]).copied().unwrap_or_default();
                Update::new(columns[0], old, columns[1])
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let cmd = Cmd::new(["sudo", "snap", "refresh"]).args(pick.keys().to_vec());
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        let cmd = Cmd::new(["sudo", "snap", "remove"]).args(pick.keys().to_vec());
        run_removals(ctx, vec![cmd])
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- Nix
/// Профіль користувача. Системні пакунки NixOS задаються в конфігурації,
/// тож UPM їх не чіпає.
pub struct Nix;

/// Формат профілю. Команди двох форматів не можна змішувати.
#[derive(Clone, Copy)]
enum Profile {
    /// Створений `nix profile`.
    Modern,
    /// Створений `nix-env`.
    Legacy,
}

fn profile(ctx: &Context) -> Option<Profile> {
    [".nix-profile", ".local/state/nix/profiles/profile"]
        .iter()
        .find_map(|dir| {
            let dir = ctx.in_home(dir);
            if dir.join("manifest.json").exists() {
                Some(Profile::Modern)
            } else if dir.join("manifest.nix").exists() {
                Some(Profile::Legacy)
            } else {
                None
            }
        })
}

fn nix_profile(args: &[&str]) -> Cmd {
    // Без цього прапорця типова установка Nix команду не знає.
    query(["nix", "--extra-experimental-features", "nix-command flakes", "profile"])
        .args(args.iter().copied())
}

/// "назва-1.2" -> назва й версія: версія починається з цифри після дефіса.
fn split_derivation(name: &str) -> (&str, &str) {
    name.match_indices('-')
        .find(|(at, _)| name[at + 1..].starts_with(|c: char| c.is_ascii_digit()))
        .map_or((name, ""), |(at, _)| (&name[..at], &name[at + 1..]))
}

/// Елементи `nix profile list --json`. У форматі 3 вони названі, у старших —
/// список, де пакунок адресують номером.
fn modern_items(data: &Value) -> Vec<Item> {
    let item = |name: Option<&str>, element: &Value, index: usize| {
        let store = element["storePaths"][0].as_str().unwrap_or_default();
        // "/nix/store/<32 знаки хешу>-назва-версія"
        let derivation = store.rsplit('/').next().unwrap_or_default();
        let derivation = derivation.split_once('-').map_or("", |(_, rest)| rest);
        let (from_store, version) = split_derivation(derivation);
        match name {
            Some(name) => Item::new(name, version),
            None => {
                let attribute = element["attrPath"]
                    .as_str()
                    .and_then(|path| path.rsplit('.').next());
                Item::new(attribute.unwrap_or(from_store), version).key(index.to_string())
            }
        }
        .info(store)
    };
    match &data["elements"] {
        Value::Object(elements) => elements
            .iter()
            .map(|(name, element)| item(Some(name), element, 0))
            .collect(),
        Value::Array(elements) => elements
            .iter()
            .enumerate()
            .map(|(index, element)| item(None, element, index))
            .collect(),
        _ => Vec::new(),
    }
}

/// `nix-env -q --json`.
fn legacy_items(data: &Value) -> Vec<Item> {
    let mut items: Vec<Item> = data
        .as_object()
        .into_iter()
        .flat_map(|packages| packages.values())
        .map(|package| {
            let field = |key: &str| package[key].as_str().unwrap_or_default();
            let (name, version) = split_derivation(field("name"));
            let name = Some(field("pname")).filter(|v| !v.is_empty()).unwrap_or(name);
            let version = Some(field("version")).filter(|v| !v.is_empty()).unwrap_or(version);
            Item::new(name, version)
        })
        .collect();
    items.sort_by_key(|item| item.name.to_lowercase());
    items
}

/// Рядки "upgrading 'a-1' to 'a-2'".
fn parse_nix_env_upgrades(output: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    output.lines().filter_map(|line| {
        let (old, new) = line
            .strip_prefix("upgrading '")?
            .strip_suffix('\'')?
            .split_once("' to '")?;
        let (name, current) = split_derivation(old);
        Some((name, current, split_derivation(new).1))
    })
}

impl Source for Nix {
    fn id(&self) -> &'static str {
        "nix"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.have("nix") || ctx.have("nix-env")
    }

    fn inventory(&self, ctx: &mut Context, _all: bool, _versions: bool) -> Query<Item> {
        let Some(format) = profile(ctx) else {
            return Query::new();
        };
        let list = match format {
            Profile::Modern => nix_profile(&["list", "--json"]),
            Profile::Legacy => query(["nix-env", "-q", "--json"]),
        };
        let data = ctx
            .read(&list)
            .and_then(|json| serde_json::from_str::<Value>(&json).ok());
        let Some(data) = data else {
            return Query::failure();
        };
        match format {
            Profile::Modern => modern_items(&data),
            Profile::Legacy => legacy_items(&data),
        }
        .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        match profile(ctx) {
            None => Query::new(),
            // `nix profile upgrade --dry-run` не називає версій.
            Some(Profile::Modern) => {
                ctx.diagnostic("nix: nix profile не вміє показати нові версії");
                let installed = self.inventory(ctx, true, false);
                let mut query: Query<Update> = Query::new();
                query.failed = installed.failed;
                query.unchecked = installed
                    .rows
                    .into_iter()
                    .map(|item| item.key)
                    .filter(|key| pick.has(key))
                    .collect();
                query
            }
            Some(Profile::Legacy) => {
                // Перелік оновлень nix-env пише в stderr.
                let preview = query(["nix-env", "--upgrade", "--dry-run"]).quiet();
                let result = ctx.capture(&preview);
                if !result.ok() {
                    ctx.diagnostic(result.stderr.trim());
                    return Query::failure();
                }
                parse_nix_env_upgrades(&result.stderr)
                    .filter(|(name, _, _)| pick.has(name))
                    .map(|(name, current, latest)| Update::new(name, current, latest))
                    .collect::<Vec<_>>()
                    .into()
            }
        }
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let cmd = match profile(ctx) {
            None => return Status::Skipped,
            Some(Profile::Modern) if pick.targeted() => nix_profile(&["upgrade"]),
            Some(Profile::Modern) => nix_profile(&["upgrade", "--all"]),
            Some(Profile::Legacy) => Cmd::new(["nix-env", "--upgrade"]),
        };
        Status::from_success(ctx.run(&cmd.args(pick.keys().to_vec())))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        let cmd = match profile(ctx) {
            None => return Status::Skipped,
            Some(Profile::Modern) => nix_profile(&["remove"]),
            Some(Profile::Legacy) => Cmd::new(["nix-env", "--uninstall"]),
        };
        run_removals(ctx, vec![cmd.args(pick.keys().to_vec())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    fn names(items: &[Item]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    // ------------------------------------------------------------ Snap
    fn snaps() -> Host {
        let mut host = Host::new("ID=ubuntu\n");
        let list = print("snap/list.txt");
        let refresh = print("snap/refresh-list.txt");
        host.file("run/snapd.socket", "");
        host.sudo().stub(
            "snap",
            &format!("case \"$*\" in list) {list} ;; 'refresh --list') {refresh} ;; esac"),
        );
        host
    }

    #[test]
    fn snap_needs_the_tool_and_a_daemon() {
        assert!(Snap.available(&snaps().ctx));
        assert!(!Snap.available(&Host::with("ID=ubuntu\n", &["snap"]).ctx));
        let mut host = Host::new("ID=ubuntu\n");
        host.file("run/snapd.socket", "");
        assert!(!Snap.available(&host.ctx));
    }

    #[test]
    fn snap_hides_bases_and_runtimes_by_default() {
        let mut host = snaps();
        let apps = Snap.inventory(&mut host.ctx, false, false);
        assert_eq!(names(&apps.rows), ["alacritty", "vlc", "yt-dlp"]);
        assert_eq!(apps.rows[1].version, "3.0.18");
        let all = Snap.inventory(&mut host.ctx, true, false);
        assert_eq!(all.rows.len(), 8);
    }

    #[test]
    fn snap_check_compares_with_installed_and_needs_no_sudo() {
        let mut host = snaps();
        let result = Snap.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed);
        let rows: Vec<_> = result
            .rows
            .iter()
            .map(|u| (u.name.as_str(), u.current.as_str(), u.latest.as_str()))
            .collect();
        assert_eq!(rows[1], ("get-iplayer", "-", "3.26"));
        assert_eq!(rows.len(), 3);
        assert!(!host.calls().iter().any(|call| call.starts_with("sudo")));

        host.stub("snap", "echo 'All snaps up to date.' >&2");
        let result = Snap.outdated(&mut host.ctx, Pick(None));
        assert_eq!((result.rows.len(), result.failed), (0, false));
        host.stub("snap", "exit 1");
        assert!(Snap.outdated(&mut host.ctx, Pick(None)).failed);
        assert!(Snap.inventory(&mut host.ctx, false, false).failed);
    }

    #[test]
    fn snap_changes_only_what_is_selected() {
        let mut host = snaps();
        host.ctx.dry = true;
        let keys = picked(&["vlc"]);
        assert!(Snap.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
        assert!(Snap.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
        assert!(Snap.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
        assert!(Snap.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert_eq!(
            host.commands(),
            ["sudo snap refresh", "sudo snap refresh vlc", "sudo snap remove vlc"]
        );
        assert!(Snap.can_remove() && Snap.needs_sudo());
    }

    // ------------------------------------------------------------ Nix
    const NIX: &str = "nix --extra-experimental-features nix-command flakes profile";

    /// Профіль у потрібному форматі; справжній вивід nix і nix-env.
    fn nix(manifest: &str) -> Host {
        let mut host = Host::new("ID=nixos\n");
        let profile = host.dir.join("home/.nix-profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join(manifest), "").unwrap();
        let listing = print("nix/profile-list.json");
        let query = print("nix/env-query.json");
        let dry = print("nix/env-upgrade-dry.txt");
        host.stub("nix", &format!("case \"$*\" in *'list --json') {listing} ;; esac"))
            .stub(
                "nix-env",
                &format!("case \"$*\" in '-q --json') {query} ;; *--dry-run) {dry} >&2 ;; esac"),
            );
        host
    }

    #[test]
    fn nix_is_available_wherever_it_is_installed() {
        for tool in ["nix", "nix-env"] {
            assert!(Nix.available(&Host::with("ID=nixos\n", &[tool]).ctx));
            assert!(Nix.available(&Host::with("ID=fedora\n", &[tool]).ctx));
        }
        assert!(!Nix.available(&Host::with("ID=nixos\n", &[]).ctx));
    }

    #[test]
    fn modern_profile_is_read_with_nix_profile() {
        let mut host = nix("manifest.json");
        let listed = Nix.inventory(&mut host.ctx, false, false);
        let rows: Vec<_> = listed
            .rows
            .iter()
            .map(|item| (item.key.as_str(), item.version.as_str()))
            .collect();
        assert_eq!(rows, [("hello", "2.12.1"), ("tree", "2.1.1")]);
        assert_eq!(host.calls(), [format!("{NIX} list --json")]);
    }

    #[test]
    fn old_modern_profiles_are_addressed_by_index() {
        let mut host = nix("manifest.json");
        let old = r#"{"version":2,"elements":[
            {"attrPath":"legacyPackages.x86_64-linux.ripgrep",
             "storePaths":["/nix/store/abcdefghijklmnopqrstuvwxyz012345-ripgrep-14.1.0"]},
            {"storePaths":["/nix/store/abcdefghijklmnopqrstuvwxyz012345-my-tool-0.3"]}]}"#;
        host.stub("nix", &format!("echo '{old}'"));
        let listed = Nix.inventory(&mut host.ctx, false, false);
        let rows: Vec<_> = listed
            .rows
            .iter()
            .map(|item| (item.name.as_str(), item.version.as_str(), item.key.as_str()))
            .collect();
        assert_eq!(rows, [("ripgrep", "14.1.0", "0"), ("my-tool", "0.3", "1")]);
    }

    #[test]
    fn legacy_profile_is_read_with_nix_env() {
        let mut host = nix("manifest.nix");
        let listed = Nix.inventory(&mut host.ctx, false, false);
        assert_eq!(names(&listed.rows), ["hello", "tree"]);
        assert_eq!(listed.rows[1].version, "2.1.1");
        assert_eq!(host.calls(), ["nix-env -q --json"]);
    }

    #[test]
    fn xdg_profile_location_is_found_too() {
        let mut host = Host::new("ID=nixos\n");
        let profile = host.dir.join("home/.local/state/nix/profiles/profile");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("manifest.json"), "").unwrap();
        host.stub("nix", &print("nix/profile-list.json"));
        assert_eq!(Nix.inventory(&mut host.ctx, false, false).rows.len(), 2);
    }

    #[test]
    fn no_profile_means_nothing_installed_and_nothing_run() {
        let mut host = Host::with("ID=nixos\n", &["nix", "nix-env"]);
        let listed = Nix.inventory(&mut host.ctx, false, false);
        assert!(listed.rows.is_empty() && !listed.failed);
        assert!(Nix.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(host.calls().is_empty() && host.commands().is_empty());
    }

    #[test]
    fn legacy_check_previews_upgrades() {
        let mut host = nix("manifest.nix");
        let result = Nix.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed && result.unchecked.is_empty());
        let update = &result.rows[0];
        assert_eq!(
            (update.name.as_str(), update.current.as_str(), update.latest.as_str()),
            ("tree", "2.1.1", "2.1.3")
        );
        assert_eq!(host.calls(), ["nix-env --upgrade --dry-run"]);
        host.stub("nix-env", "exit 1");
        assert!(Nix.outdated(&mut host.ctx, Pick(None)).failed);
    }

    #[test]
    fn modern_profile_is_unchecked_rather_than_up_to_date() {
        let mut host = nix("manifest.json");
        let result = Nix.outdated(&mut host.ctx, Pick(None));
        assert!(result.rows.is_empty() && !result.failed);
        assert_eq!(result.unchecked, ["hello", "tree"]);
    }

    #[test]
    fn nix_formats_are_never_mixed_and_never_use_sudo() {
        let keys = picked(&["tree"]);
        for (manifest, expected) in [
            (
                "manifest.json",
                [
                    format!("{NIX} upgrade --all"),
                    format!("{NIX} upgrade tree"),
                    format!("{NIX} remove tree"),
                ],
            ),
            (
                "manifest.nix",
                [
                    "nix-env --upgrade".to_string(),
                    "nix-env --upgrade tree".to_string(),
                    "nix-env --uninstall tree".to_string(),
                ],
            ),
        ] {
            let mut host = nix(manifest);
            host.ctx.dry = true;
            assert!(Nix.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert!(Nix.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Nix.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Nix.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert_eq!(host.commands(), expected);
        }
        assert!(Nix.can_remove() && !Nix.needs_sudo());
    }
}
