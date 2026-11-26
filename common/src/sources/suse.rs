//! openSUSE та SLE: Zypper.

use std::collections::BTreeSet;

use crate::context::{Cmd, Context};
use crate::platform::Family;
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::rpm::rpm_packages;
use crate::sources::query;

pub struct Zypper;

/// Коди успіху: 102 — бажано перезавантажити, 103 — оновився сам менеджер,
/// і решту латок застосує повторний запуск.
const REBOOT: i32 = 102;
const RESTART: i32 = 103;

/// Рядки таблиці zypper, перший стовпець яких (стан) починається з `status`.
fn table<'a>(listing: &'a str, status: &'a str) -> impl Iterator<Item = Vec<&'a str>> {
    listing
        .lines()
        .map(|line| line.split('|').map(str::trim).collect::<Vec<_>>())
        .filter(move |columns| columns.len() >= 5 && columns[0].starts_with(status))
}

impl Zypper {
    fn change(&self, ctx: &mut Context, verb: &str, packages: &[String]) -> Status {
        let mut argv = vec!["sudo", "zypper"];
        // Глобальний прапорець zypper стоїть перед дією.
        if ctx.yes {
            argv.push("--non-interactive");
        }
        argv.push(verb);
        let cmd = Cmd::new(argv)
            .args(packages.to_vec())
            .ok_codes(&[REBOOT, RESTART]);
        match ctx.run_status(&cmd) {
            Some(REBOOT) => ctx.note("zypper: бажано перезавантажити комп'ютер".to_string()),
            Some(RESTART) => ctx.note(
                "zypper оновив сам себе: запустіть оновлення ще раз, щоб доставити решту"
                    .to_string(),
            ),
            Some(_) => {}
            None => return Status::Failed,
        }
        Status::Ok
    }
}

impl Source for Zypper {
    fn id(&self) -> &'static str {
        "zypper"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.manages(Family::Suse) && ctx.have("zypper") && ctx.have("rpm")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(mut packages) = rpm_packages(ctx) else {
            return Query::failure();
        };
        if !all {
            let userinstalled = query(["zypper", "--non-interactive", "--no-refresh"])
                .args(["packages", "--installed-only", "--userinstalled"])
                .quiet();
            match ctx.read(&userinstalled) {
                Some(listing) => {
                    let user: BTreeSet<&str> =
                        table(&listing, "i").map(|columns| columns[2]).collect();
                    packages.retain(|(name, _)| user.contains(name.as_str()));
                }
                None => ctx.diagnostic(
                    "zypper не каже, що встановили ви (потрібна новіша версія) — показано все",
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
        if !ctx.dry && !ctx.refresh(&Cmd::new(["sudo", "zypper", "refresh"]).interactive()) {
            return Query::failure();
        }
        let list = query(["zypper", "--non-interactive", "--no-refresh", "list-updates"]);
        let Some(listing) = ctx.read(&list) else {
            return Query::failure();
        };
        // Стовпці: стан, репозиторій, назва, наявна, нова, архітектура.
        table(&listing, "v")
            .filter(|columns| columns.len() >= 6 && pick.has(columns[2]))
            .map(|columns| Update::new(columns[2], columns[3], columns[4]))
            .collect::<Vec<_>>()
            .into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        // На безперервних випусках усе разом оновлюють лише через dup.
        let verb = if ctx.platform.rolling && !pick.targeted() {
            "dup"
        } else {
            "update"
        };
        self.change(ctx, verb, pick.keys())
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        self.change(ctx, "remove", pick.keys())
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    const LEAP: &str = "ID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n";
    const TUMBLEWEED: &str =
        include_str!("../../tests/fixtures/zypper-tumbleweed/os-release.txt");

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    /// Leap 15.6: справжній вивід zypper і rpm.
    fn leap() -> Host {
        let mut host = Host::new(LEAP);
        let user = print("zypper/userinstalled.txt");
        let updates = print("zypper/list-updates.txt");
        host.sudo()
            .stub(
                "zypper",
                &format!("case \"$*\" in *userinstalled*) {user} ;; *list-updates*) {updates} ;; esac"),
            )
            .stub("rpm", &print("zypper/rpm-all.txt"));
        host
    }

    fn find<'a>(items: &'a [Item], name: &str) -> Option<&'a Item> {
        items.iter().find(|item| item.name == name)
    }

    #[test]
    fn zypper_is_available_on_a_mutable_suse_host() {
        let tools = ["zypper", "rpm"];
        assert!(Zypper.available(&Host::with(LEAP, &tools).ctx));
        assert!(Zypper.available(&Host::with(TUMBLEWEED, &tools).ctx));
        assert!(!Zypper.available(&Host::with("ID=fedora\n", &tools).ctx));
        assert!(!Zypper.available(&Host::with("ID=arch\n", &tools).ctx));
        assert!(!Zypper.available(&Host::with("ID=opensuse-microos\n", &tools).ctx));
        assert!(!Zypper.available(&Host::with(LEAP, &["rpm"]).ctx));
    }

    #[test]
    fn zypper_lists_user_installed_by_default_and_all_on_request() {
        let mut host = leap();
        let user = Zypper.inventory(&mut host.ctx, false, false);
        assert!(!user.failed);
        let base = find(&user.rows, "aaa_base").unwrap();
        assert_eq!(base.version, "84.87+git20180409.04c9dae-150300.10.28.2");
        let all = Zypper.inventory(&mut host.ctx, true, false);
        assert!(all.rows.len() > user.rows.len() * 3, "{}", all.rows.len());
        assert!(!user.rows.is_empty() && user.rows.len() < 30);
        assert!(find(&user.rows, "Name").is_none(), "заголовок таблиці");
    }

    #[test]
    fn old_zypper_lists_everything_and_says_why() {
        let mut host = Host::new(LEAP);
        host.stub("zypper", "exit 2")
            .stub("rpm", &print("zypper/rpm-all.txt"));
        let listed = Zypper.inventory(&mut host.ctx, false, false);
        assert!(!listed.failed);
        assert!(listed.rows.len() > 130);
        assert!(host.messages().contains("zypper"), "{}", host.messages());
    }

    #[test]
    fn zypper_reports_updates() {
        let mut host = leap();
        let result = Zypper.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed);
        let rows: Vec<_> = result
            .rows
            .iter()
            .map(|u| (u.name.as_str(), u.current.as_str(), u.latest.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("less", "643-150600.1.37", "643-150600.3.3.1"),
                ("tar", "1.34-150000.3.34.1", "1.34-150000.3.37.1"),
            ]
        );
        assert_eq!(
            host.calls()[..2],
            [
                "sudo zypper refresh",
                "zypper refresh",
            ]
        );
        let keys = picked(&["tar"]);
        assert_eq!(Zypper.outdated(&mut host.ctx, Pick(Some(&keys))).rows.len(), 1);

        let mut host = Host::new(TUMBLEWEED);
        host.sudo()
            .stub("zypper", &print("zypper-tumbleweed/list-updates.txt"));
        let result = Zypper.outdated(&mut host.ctx, Pick(None));
        assert_eq!(result.rows[0].name, "openSUSE-build-key");
        assert_eq!(result.rows[0].latest, "1.0-69.1");
    }

    #[test]
    fn zypper_dry_check_needs_no_sudo() {
        let mut host = leap();
        host.ctx.dry = true;
        assert_eq!(Zypper.outdated(&mut host.ctx, Pick(None)).rows.len(), 2);
        assert_eq!(
            host.calls(),
            ["zypper --non-interactive --no-refresh list-updates"]
        );
    }

    #[test]
    fn zypper_no_updates_is_fine_and_failed_refresh_is_not() {
        let mut host = Host::new(LEAP);
        host.sudo().stub("zypper", "echo 'No updates found.'");
        let result = Zypper.outdated(&mut host.ctx, Pick(None));
        assert_eq!((result.rows.len(), result.failed), (0, false));
        host.stub("zypper", "case $1 in refresh) exit 4 ;; esac");
        assert!(Zypper.outdated(&mut host.ctx, Pick(None)).failed);
    }

    #[test]
    fn full_upgrade_is_dup_only_on_rolling_releases() {
        for (release, verb) in [(TUMBLEWEED, "dup"), (LEAP, "update")] {
            for (yes, flag) in [(true, "--non-interactive "), (false, "")] {
                let mut host = Host::new(release);
                host.ctx.dry = true;
                host.ctx.yes = yes;
                let keys = picked(&["less"]);
                assert!(Zypper.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
                assert!(Zypper.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
                assert!(Zypper.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
                assert!(Zypper.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
                assert_eq!(
                    host.commands(),
                    [
                        format!("sudo zypper {flag}{verb}"),
                        format!("sudo zypper {flag}update less"),
                        format!("sudo zypper {flag}remove less"),
                    ]
                );
            }
        }
        assert!(Zypper.can_remove() && Zypper.needs_sudo());
    }

    #[test]
    fn reboot_and_restart_codes_are_success_with_a_note() {
        for (code, ok, note) in [
            (0, true, None),
            (102, true, Some("перезавантаж")),
            (103, true, Some("ще раз")),
            (4, false, None),
        ] {
            let mut host = Host::new(LEAP);
            host.sudo().stub("zypper", &format!("exit {code}"));
            let status = Zypper.upgrade(&mut host.ctx, Pick(None));
            assert_eq!(status == Status::Ok, ok, "код {code}");
            assert_eq!(host.ctx.failed_commands.is_empty(), ok);
            match note {
                Some(note) => assert!(host.ctx.notes[0].contains(note)),
                None => assert!(host.ctx.notes.is_empty()),
            }
        }
    }
}
