//! Debian, Ubuntu та похідні: APT.

use std::collections::{BTreeMap, BTreeSet};

use crate::context::{Cmd, Context};
use crate::platform::Family;
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::{query, run_removals};

pub struct Apt;

/// Стан, пакунок, версія, архітектура.
const INSTALLED_FORMAT: &str = "${db:Status-Status}\\t${Package}\\t${Version}\\t${Architecture}\\n";

/// Встановлене: ключ → версія. Ключ чужої архітектури — "назва:арх", як у
/// `apt-mark` та `apt-get`; своєї та "all" — лише назва.
fn installed(ctx: &Context) -> Option<BTreeMap<String, String>> {
    let native = ctx.read(&query(["dpkg", "--print-architecture"]))?;
    let listing = ctx.read(&query(["dpkg-query", "-W", "-f", INSTALLED_FORMAT]))?;
    let packages = listing.lines().filter_map(|line| {
        let fields: Vec<&str> = line.split('\t').collect();
        let ["installed", name, version, arch] = fields[..] else {
            return None;
        };
        let key = if arch == "all" || arch == native.trim() {
            name.to_string()
        } else {
            format!("{name}:{arch}")
        };
        Some((key, version.to_string()))
    });
    Some(packages.collect())
}

/// Оновлення з `apt-get -s`: "Inst назва [стара] (нова репозиторій [арх])".
/// Рядки без старої версії — нові залежності, а не оновлення.
fn parse_simulation(output: &str) -> Vec<(&str, &str, &str)> {
    output
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.strip_prefix("Inst ")?.split_once(' ')?;
            let (current, rest) = rest.strip_prefix('[')?.split_once("] (")?;
            let latest = rest.split_whitespace().next()?;
            Some((name, current, latest))
        })
        .collect()
}

/// `apt-get` через sudo. Без термінала з `-y` не чекає на запитання й лишає
/// змінені вами файли налаштувань. `env` потрібен, бо sudo скидає змінні.
fn apt_get(ctx: &Context, action: &[&str], packages: &[String]) -> Cmd {
    let mut cmd = if ctx.yes && !ctx.interactive() {
        Cmd::new([
            "sudo",
            "env",
            "DEBIAN_FRONTEND=noninteractive",
            "apt-get",
            "-o",
            "Dpkg::Options::=--force-confdef",
            "-o",
            "Dpkg::Options::=--force-confold",
        ])
    } else {
        Cmd::new(["sudo", "apt-get"])
    };
    cmd = cmd.args(action.iter().copied());
    if ctx.yes {
        cmd = cmd.args(["-y"]);
    }
    cmd.args(packages.to_vec())
}

/// Ті з названих пакунків, що зараз позначені як установлені залежностями.
fn automatic(ctx: &Context, packages: &[String]) -> Vec<String> {
    let listing = ctx
        .read(&query(["apt-mark", "showauto"]))
        .unwrap_or_default();
    let auto: BTreeSet<&str> = listing.lines().map(str::trim).collect();
    packages
        .iter()
        .filter(|name| auto.contains(name.as_str()))
        .cloned()
        .collect()
}

fn refresh() -> Cmd {
    Cmd::new(["sudo", "apt-get", "update"]).interactive()
}

impl Source for Apt {
    fn id(&self) -> &'static str {
        "apt"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.manages(Family::Debian) && ctx.have("apt-get") && ctx.have("dpkg-query")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(mut packages) = installed(ctx) else {
            return Query::failure();
        };
        if !all {
            match ctx.read(&query(["apt-mark", "showmanual"])) {
                Some(manual) => {
                    let manual: BTreeSet<&str> = manual.lines().map(str::trim).collect();
                    packages.retain(|key, _| manual.contains(key.as_str()));
                }
                None => ctx.diagnostic(
                    "apt: apt-mark не каже, що встановили ви, — показано все",
                ),
            }
        }
        packages
            .into_iter()
            .map(|(key, version)| Item::new(key, version))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        if !ctx.dry && !ctx.refresh(&refresh()) {
            return Query::failure();
        }
        // Симуляція показує саме те, що зробить upgrade; sudo їй не потрібен.
        let simulate = query(["apt-get", "-s", "upgrade", "--with-new-pkgs"]);
        let Some(output) = ctx.read(&simulate) else {
            return Query::failure();
        };
        parse_simulation(&output)
            .into_iter()
            .filter(|(name, _, _)| pick.has(name))
            .map(|(name, current, latest)| Update::new(name, current, latest))
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if !ctx.run(&refresh()) {
            return Status::Failed;
        }
        // Жоден із варіантів не видаляє встановлених пакунків.
        let action: &[&str] = if pick.targeted() {
            &["install", "--only-upgrade", "--no-remove"]
        } else {
            &["upgrade", "--with-new-pkgs"]
        };
        // `apt-get install` позначає названі пакунки встановленими вручну, тож
        // залежностям потім повертаємо позначку, щоб autoremove їх бачив.
        let dependencies = automatic(ctx, pick.keys());
        let cmd = apt_get(ctx, action, pick.keys());
        let mut success = ctx.run(&cmd);
        if !dependencies.is_empty() {
            let restore = Cmd::new(["sudo", "apt-mark", "auto"]).args(dependencies);
            success &= ctx.run(&restore);
        }
        Status::from_success(success)
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        let cmd = apt_get(ctx, &["remove"], pick.keys());
        run_removals(ctx, vec![cmd])
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    const DEBIAN: &str = "ID=debian\n";

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    /// Debian із бібліотекою двох архітектур; справжній вивід.
    fn debian() -> Host {
        let mut host = Host::new(DEBIAN);
        let installed = print("apt/installed.tsv");
        host.sudo()
            .stub("dpkg", &print("apt/native-arch.txt"))
            .stub(
                "dpkg-query",
                // Видалений пакунок із залишеними налаштуваннями — не встановлений.
                &format!("{installed}; printf 'config-files\\tgone\\t1.0\\tamd64\\n'"),
            )
            .stub("apt-mark", &print("apt/manual.txt"))
            .stub(
                "apt-get",
                &format!("case \"$1\" in -s) {} ;; esac", print("apt/simulate-upgrade.txt")),
            );
        host
    }

    fn rows(items: &[Item]) -> Vec<(&str, &str)> {
        items
            .iter()
            .map(|item| (item.key.as_str(), item.version.as_str()))
            .collect()
    }

    #[test]
    fn apt_is_available_on_a_mutable_debian_host() {
        let tools = ["apt-get", "dpkg-query"];
        assert!(Apt.available(&Host::with(DEBIAN, &tools).ctx));
        assert!(Apt.available(&Host::with("ID=ubuntu\nID_LIKE=debian\n", &tools).ctx));
        assert!(!Apt.available(&Host::with("ID=fedora\n", &tools).ctx));
        assert!(!Apt.available(&Host::with("ID=arch\n", &tools).ctx));
        assert!(!Apt.available(&Host::with(DEBIAN, &["dpkg-query"]).ctx));
        let mut atomic = Host::with(DEBIAN, &tools);
        atomic.file("run/ostree-booted", "").reprobe();
        assert!(!Apt.available(&atomic.ctx));
    }

    #[test]
    fn apt_default_listing_has_only_manually_installed() {
        let mut host = debian();
        let listed = Apt.inventory(&mut host.ctx, false, false);
        assert!(!listed.failed);
        let keys: Vec<&str> = listed.rows.iter().map(|item| item.key.as_str()).collect();
        assert_eq!(keys, ["bash", "hello", "liblzma5", "liblzma5:i386"]);
    }

    #[test]
    fn apt_full_listing_keeps_architectures_apart() {
        let mut host = debian();
        let listed = Apt.inventory(&mut host.ctx, true, false);
        let rows = rows(&listed.rows);
        assert!(rows.len() > 90);
        assert!(rows.contains(&("liblzma5", "5.4.1-1+deb12u1")));
        assert!(rows.contains(&("liblzma5:i386", "5.4.1-1+deb12u1")));
        // Пакунки "для всіх архітектур" — без суфікса.
        assert!(rows.iter().any(|(name, _)| *name == "debconf"));
        assert!(!rows.iter().any(|(name, _)| *name == "gone"));
    }

    #[test]
    fn apt_without_manual_marks_lists_everything_and_says_why() {
        let mut host = debian();
        host.stub("apt-mark", "exit 100");
        let listed = Apt.inventory(&mut host.ctx, false, false);
        assert!(listed.rows.len() > 90);
        assert!(host.messages().contains("apt-mark"));
    }

    #[test]
    fn apt_check_refreshes_then_simulates() {
        let mut host = debian();
        let result = Apt.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed);
        assert_eq!(
            host.calls()[..3],
            [
                "sudo apt-get update",
                "apt-get update",
                "apt-get -s upgrade --with-new-pkgs",
            ]
        );
        let find = |key: &str| {
            result
                .rows
                .iter()
                .find(|update| update.key == key)
                .map(|update| (update.current.as_str(), update.latest.as_str()))
        };
        let lzma = Some(("5.4.1-1+deb12u1", "5.4.1-1+deb12u2"));
        assert_eq!(find("liblzma5"), lzma);
        assert_eq!(find("liblzma5:i386"), lzma);
        assert!(result.rows.len() > 30);

        let keys = picked(&["liblzma5:i386"]);
        assert_eq!(Apt.outdated(&mut host.ctx, Pick(Some(&keys))).rows.len(), 1);
    }

    #[test]
    fn new_dependencies_are_not_updates() {
        let simulated = "Inst brand-new (1.0 Debian:12 [amd64])\n\
                         Inst old [1] (2 Debian:12 [amd64]) [x:amd64 ]\n\
                         Conf old (2 Debian:12 [amd64])\n";
        let updates = parse_simulation(simulated);
        assert_eq!(updates, [("old", "1", "2")]);
    }

    #[test]
    fn apt_dry_check_needs_no_sudo() {
        let mut host = debian();
        host.ctx.dry = true;
        assert!(!Apt.outdated(&mut host.ctx, Pick(None)).rows.is_empty());
        assert!(!host.calls().iter().any(|call| call.starts_with("sudo")));
    }

    #[test]
    fn apt_up_to_date_is_empty_and_failed_refresh_is_reported() {
        let mut host = debian();
        host.stub(
            "apt-get",
            &format!("case \"$1\" in -s) {} ;; esac", print("apt/simulate-none.txt")),
        );
        let result = Apt.outdated(&mut host.ctx, Pick(None));
        assert_eq!((result.rows.len(), result.failed), (0, false));
        host.stub("apt-get", "case \"$1\" in update) exit 100 ;; esac");
        assert!(Apt.outdated(&mut host.ctx, Pick(None)).failed);
    }

    #[test]
    fn apt_changes_in_a_terminal() {
        for (yes, flag) in [(true, " -y"), (false, "")] {
            let mut host = Host::new(DEBIAN);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            let keys = picked(&["hello", "liblzma5:i386"]);
            assert!(Apt.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert!(Apt.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Apt.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Apt.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert_eq!(
                host.commands(),
                [
                    "sudo apt-get update".to_string(),
                    format!("sudo apt-get upgrade --with-new-pkgs{flag}"),
                    "sudo apt-get update".to_string(),
                    format!(
                        "sudo apt-get install --only-upgrade --no-remove{flag} hello liblzma5:i386"
                    ),
                    format!("sudo apt-get remove{flag} hello liblzma5:i386"),
                ]
            );
        }
        assert!(Apt.can_remove() && Apt.needs_sudo());
    }

    #[test]
    fn targeted_upgrade_keeps_dependencies_marked_as_automatic() {
        // `apt-get install` позначає названий пакунок "встановленим вручну".
        for (upgrade_code, status) in [(0, Status::Ok), (100, Status::Failed)] {
            let mut host = Host::new(DEBIAN);
            host.ctx.yes = true;
            host.sudo()
                .stub("apt-mark", "[ $1 = showauto ] && echo libdep; exit 0")
                .stub(
                    "apt-get",
                    &format!("case $1 in install) exit {upgrade_code} ;; esac"),
                );
            let keys = picked(&["hello", "libdep"]);
            assert!(Apt.upgrade(&mut host.ctx, Pick(Some(&keys))) == status);
            assert_eq!(
                host.commands().last().map(String::as_str),
                Some("sudo apt-mark auto libdep")
            );
        }
        // Нічого позначати назад — нічого зайвого не запускається.
        let mut host = Host::new(DEBIAN);
        host.ctx.dry = true;
        host.stub("apt-mark", "exit 0");
        let keys = picked(&["hello"]);
        Apt.upgrade(&mut host.ctx, Pick(Some(&keys)));
        assert!(!host.commands().iter().any(|command| command.contains("apt-mark")));
    }

    #[test]
    fn apt_without_terminal_never_asks_and_keeps_local_configs() {
        let mut host = Host::new(DEBIAN);
        host.headless();
        host.ctx.dry = true;
        host.ctx.yes = true;
        let keys = picked(&["hello"]);
        Apt.upgrade(&mut host.ctx, Pick(None));
        Apt.remove(&mut host.ctx, Pick(Some(&keys)));
        let quiet = "sudo env DEBIAN_FRONTEND=noninteractive apt-get \
                     -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold";
        assert_eq!(
            host.commands()[1..],
            [
                format!("{quiet} upgrade --with-new-pkgs -y"),
                format!("{quiet} remove -y hello"),
            ]
        );
    }

    #[test]
    fn essential_removal_is_a_failure_and_is_not_forced() {
        let mut host = Host::new(DEBIAN);
        host.sudo().stub("apt-get", "exit 100");
        host.ctx.yes = true;
        let keys = picked(&["bash"]);
        assert!(Apt.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Failed);
        assert_eq!(host.calls(), ["sudo apt-get remove -y bash", "apt-get remove -y bash"]);
    }
}
