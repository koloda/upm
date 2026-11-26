//! Alpine: APK.

use std::collections::{BTreeMap, BTreeSet};

use crate::context::{Cmd, Context};
use crate::platform::Family;
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::{confirmable, query, run_removals};

pub struct Apk;

/// "назва-версія-rN" -> назва й "версія-rN".
fn split_package(package: &str) -> Option<(&str, &str)> {
    let (head, _release) = package.rsplit_once('-')?;
    let (name, _version) = head.rsplit_once('-')?;
    Some((name, &package[name.len() + 1..]))
}

/// Назви з `/etc/apk/world`; обмеження версії та мітки репозиторію відкидаються.
fn world(ctx: &Context) -> Option<BTreeSet<String>> {
    let text = std::fs::read_to_string(ctx.platform.path("etc/apk/world")).ok()?;
    Some(
        text.split_whitespace()
            // "!назва" — заборона, а не бажаний пакунок.
            .filter(|entry| !entry.starts_with('!'))
            .filter_map(|entry| entry.split(['<', '>', '=', '~', '@']).next())
            .map(str::to_string)
            .collect(),
    )
}

/// `sudo apk [--interactive] дія пакунки`: без `-y` apk перепитує.
fn apk(ctx: &Context, action: &str, packages: &[String]) -> Cmd {
    let mut argv = vec!["sudo", "apk"];
    if !ctx.yes {
        argv.push("--interactive");
    }
    argv.push(action);
    Cmd::new(argv).args(packages.to_vec())
}

fn refresh() -> Cmd {
    Cmd::new(["sudo", "apk", "update"]).interactive()
}

impl Source for Apk {
    fn id(&self) -> &'static str {
        "apk"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.manages(Family::Alpine) && ctx.have("apk")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(listing) = ctx.read(&query(["apk", "info", "-v"])) else {
            return Query::failure();
        };
        let mut packages: BTreeMap<&str, &str> =
            listing.lines().filter_map(split_package).collect();
        if !all {
            match world(ctx) {
                Some(world) => packages.retain(|name, _| world.contains(*name)),
                None => ctx.diagnostic(
                    "apk: /etc/apk/world не читається, тож не видно, що встановили ви, — \
                     показано все",
                ),
            }
        }
        packages
            .into_iter()
            .map(|(name, version)| Item::new(name, version))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        if !ctx.dry && !ctx.refresh(&refresh()) {
            return Query::failure();
        }
        let Some(listing) = ctx.read(&query(["apk", "version", "-l", "<"])) else {
            return Query::failure();
        };
        // Рядки "назва-стара < нова".
        listing
            .lines()
            .filter_map(|line| {
                let fields: Vec<&str> = line.split_whitespace().collect();
                let [package, "<", latest] = fields[..] else {
                    return None;
                };
                let (name, current) = split_package(package)?;
                pick.has(name).then(|| Update::new(name, current, latest))
            })
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if !confirmable(ctx, self.id()) {
            return Status::Skipped;
        }
        if !ctx.run(&refresh()) {
            return Status::Failed;
        }
        // `upgrade назви` не змінює world, на відміну від `add --upgrade`.
        let cmd = apk(ctx, "upgrade", pick.keys());
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() || !confirmable(ctx, self.id()) {
            return Status::Skipped;
        }
        let cmd = apk(ctx, "del", pick.keys());
        let status = run_removals(ctx, vec![cmd]);
        if status != Status::Ok || ctx.dry {
            return status;
        }
        // `apk del` виходить успішно, навіть коли пакунок лишено, бо він
        // потрібен іншим, — тому перевіряємо.
        let check = query(["apk", "info", "-e"]).args(pick.keys().to_vec());
        let kept = ctx.capture(&check).stdout;
        let kept: Vec<&str> = kept.split_whitespace().collect();
        if kept.is_empty() {
            return status;
        }
        ctx.note(format!(
            "apk: не видалено, бо потрібні іншим пакункам: {}",
            kept.join(" ")
        ));
        Status::Failed
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    const ALPINE: &str = "ID=alpine\n";

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    /// Alpine 3.20 з оновленнями; справжній вивід.
    fn alpine() -> Host {
        let mut host = Host::new(ALPINE);
        let info = print("apk/info.txt");
        let updates = print("apk/version-updates.txt");
        host.file("etc/apk/world", include_str!("../../tests/fixtures/apk/world.txt"));
        host.sudo().stub(
            "apk",
            &format!("case \"$*\" in 'info -v') {info} ;; version*) {updates} ;; esac"),
        );
        host
    }

    fn names(items: &[Item]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    #[test]
    fn apk_is_available_on_a_mutable_alpine_host() {
        assert!(Apk.available(&Host::with(ALPINE, &["apk"]).ctx));
        assert!(!Apk.available(&Host::with("ID=fedora\n", &["apk"]).ctx));
        assert!(!Apk.available(&Host::with("ID=arch\n", &["apk"]).ctx));
        assert!(!Apk.available(&Host::with(ALPINE, &[]).ctx));
    }

    #[test]
    fn apk_lists_world_packages_by_default() {
        let mut host = alpine();
        let world = Apk.inventory(&mut host.ctx, false, false);
        assert!(!world.failed);
        assert_eq!(
            names(&world.rows),
            ["alpine-baselayout", "alpine-keys", "apk-tools", "busybox"]
        );
        assert_eq!(world.rows[2].version, "2.14.4-r0");
        let all = Apk.inventory(&mut host.ctx, true, false);
        assert_eq!(all.rows.len(), 14);
        assert!(names(&all.rows).contains(&"ca-certificates-bundle"));
    }

    #[test]
    fn world_entries_may_carry_constraints() {
        let mut host = alpine();
        host.file("etc/apk/world", "busybox>=1.36 musl@edge\nzlib=1.3.1-r1\n!ssl_client\n");
        let world = Apk.inventory(&mut host.ctx, false, false);
        assert_eq!(names(&world.rows), ["busybox", "musl", "zlib"]);
    }

    #[test]
    fn unreadable_world_lists_everything_and_says_why() {
        let mut host = alpine();
        std::fs::remove_file(host.dir.join("root/etc/apk/world")).unwrap();
        assert_eq!(Apk.inventory(&mut host.ctx, false, false).rows.len(), 14);
        assert!(host.messages().contains("world"));
    }

    #[test]
    fn apk_check_refreshes_then_compares() {
        let mut host = alpine();
        let result = Apk.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed);
        assert_eq!(result.rows.len(), 10);
        let first = &result.rows[0];
        assert_eq!(
            (first.name.as_str(), first.current.as_str(), first.latest.as_str()),
            ("apk-tools", "2.14.4-r0", "2.14.13-r0")
        );
        assert_eq!(
            host.calls()[..3],
            ["sudo apk update", "apk update", "apk version -l <"]
        );
        let keys = picked(&["zlib"]);
        assert_eq!(Apk.outdated(&mut host.ctx, Pick(Some(&keys))).rows.len(), 1);
    }

    #[test]
    fn apk_dry_check_and_failures() {
        let mut host = alpine();
        host.ctx.dry = true;
        assert_eq!(Apk.outdated(&mut host.ctx, Pick(None)).rows.len(), 10);
        assert_eq!(host.calls(), ["apk version -l <"]);

        let mut host = Host::new(ALPINE);
        host.sudo()
            .stub("apk", "case $1 in version) echo 'Installed: Available:' ;; esac");
        let result = Apk.outdated(&mut host.ctx, Pick(None));
        assert_eq!((result.rows.len(), result.failed), (0, false));
        host.stub("apk", "case $1 in update) exit 1 ;; esac");
        assert!(Apk.outdated(&mut host.ctx, Pick(None)).failed);
    }

    #[test]
    fn apk_upgrades_without_touching_world_and_asks_without_yes() {
        for (yes, flag) in [(true, ""), (false, "--interactive ")] {
            let mut host = Host::new(ALPINE);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            let keys = picked(&["musl"]);
            assert!(Apk.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert!(Apk.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Apk.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Apk.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert_eq!(
                host.commands(),
                [
                    "sudo apk update".to_string(),
                    format!("sudo apk {flag}upgrade"),
                    "sudo apk update".to_string(),
                    format!("sudo apk {flag}upgrade musl"),
                    format!("sudo apk {flag}del musl"),
                ]
            );
        }
        assert!(Apk.can_remove() && Apk.needs_sudo());
    }

    #[test]
    fn apk_does_not_change_anything_when_nobody_can_confirm() {
        // `apk --interactive` на кінці вводу виконує дію.
        let mut host = Host::new(ALPINE);
        host.headless();
        let keys = picked(&["tree"]);
        assert!(Apk.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(Apk.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Skipped);
        assert!(host.commands().is_empty());
    }

    #[test]
    fn kept_package_makes_removal_a_failure() {
        // `apk del` виходить успішно, навіть коли пакунок лишився.
        for (still_installed, expected) in [(true, false), (false, true)] {
            let mut host = Host::new(ALPINE);
            host.ctx.yes = true;
            let answer = if still_installed { "echo libcrypto3" } else { "exit 1" };
            host.sudo()
                .stub("apk", &format!("case \"$1 $2\" in 'info -e') {answer} ;; esac"));
            let keys = picked(&["libcrypto3"]);
            let status = Apk.remove(&mut host.ctx, Pick(Some(&keys)));
            assert_eq!(status == Status::Ok, expected);
            assert_eq!(host.ctx.notes.is_empty(), expected);
        }
    }
}
