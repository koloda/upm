//! Джерела для систем на RPM: DNF, YUM та лише перегляд, коли їх немає.

use std::collections::BTreeSet;

use crate::context::{Cmd, Context};
use crate::platform::Family;
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::suse::Zypper;
use crate::sources::{change_cmd, query, run_removals};

/// Усі пакунки з бази RPM: назва й версія без повторів.
pub(super) fn rpm_packages(ctx: &Context) -> Option<BTreeSet<(String, String)>> {
    let listing = ctx.read(&query(["rpm", "-qa", "--qf", "%{NAME} %{VERSION}-%{RELEASE}\\n"]))?;
    Some(
        listing
            .lines()
            .filter_map(|line| line.split_once(' '))
            .map(|(name, version)| (name.to_string(), version.to_string()))
            .collect(),
    )
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
        ctx.platform.manages(Family::Rpm) && ctx.have("dnf") && ctx.have("rpm")
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

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        let mut cmd = Cmd::new(["sudo", "dnf", "remove"]);
        if ctx.yes {
            cmd = cmd.args(["-y"]);
        }
        run_removals(ctx, vec![cmd.args(pick.keys().to_vec())])
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- YUM
/// Старі системи на RPM, де DNF ще немає.
pub struct Yum;

/// Пари "name.arch", "нова версія" з виводу `yum check-update`.
/// Довгі назви yum переносить, тому поля можуть бути на двох рядках.
fn parse_yum_updates(listing: &str) -> Vec<(String, String)> {
    let mut updates = Vec::new();
    let mut fields: Vec<&str> = Vec::new();
    for line in listing.lines().take_while(|line| !line.starts_with("Obsolet")) {
        // Новий запис починається з початку рядка.
        if !line.starts_with(char::is_whitespace) {
            fields.clear();
        }
        fields.extend(line.split_whitespace());
        if let [package, version, _repository] = fields[..]
            && package.rsplit_once('.').is_some_and(|(_, arch)| {
                ["x86_64", "noarch", "i686", "aarch64", "ppc64le", "s390x"].contains(&arch)
            })
        {
            updates.push((package.to_string(), version.to_string()));
            fields.clear();
        }
    }
    updates
}

/// Пакунки, які просив користувач; `None`, якщо дізнатися нема чим.
fn yum_user_installed(ctx: &Context) -> Option<BTreeSet<String>> {
    if !ctx.have("repoquery") {
        return None;
    }
    let reasons = ctx.read(&query(["repoquery", "--cache", "--installed", "--all"]).args([
        "--qf",
        "%{name} %{yumdb_info.reason}",
    ]))?;
    Some(
        reasons
            .lines()
            .filter_map(|line| line.split_once(' '))
            .filter(|(_, reason)| reason.trim() == "user")
            .map(|(name, _)| name.to_string())
            .collect(),
    )
}

impl Source for Yum {
    fn id(&self) -> &'static str {
        "yum"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.manages(Family::Rpm)
            && ctx.have("yum")
            && ctx.have("rpm")
            && !ctx.have("dnf")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(mut packages) = rpm_packages(ctx) else {
            return Query::failure();
        };
        if !all {
            match yum_user_installed(ctx) {
                Some(user) => packages.retain(|(name, _)| user.contains(name)),
                None => ctx.diagnostic(
                    "yum: без repoquery (пакунок yum-utils) не видно, що встановили ви — \
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
        let check = if ctx.dry {
            query(["yum", "check-update", "-q", "-C"])
        } else {
            query(["sudo", "yum", "check-update", "-q"])
                .args(["--setopt=metadata_expire=0"])
                .interactive()
        };
        let result = ctx.capture(&check);
        // 100 — "є оновлення".
        if !matches!(result.code, Some(0 | 100)) {
            return Query::failure();
        }
        let mut updates = Vec::new();
        for (package, latest) in parse_yum_updates(&result.stdout) {
            let name = package.rsplit_once('.').map_or(package.as_str(), |(name, _)| name);
            if !pick.has(name) {
                continue;
            }
            let installed = query(["rpm", "-q", "--qf"])
                .args(["%|EPOCH?{%{EPOCH}:}:{}|%{VERSION}-%{RELEASE}\\n", package.as_str()])
                .quiet();
            let current = ctx.read(&installed).unwrap_or_default();
            let current = current.lines().last().unwrap_or_default();
            let shown = package
                .trim_end_matches(".x86_64")
                .trim_end_matches(".noarch");
            updates.push(Update::new(shown, current, &latest).key(name));
        }
        updates.into()
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let cmd = change_cmd(ctx, &["sudo", "yum", "update"], "-y", pick.keys());
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if pick.keys().is_empty() {
            return Status::Skipped;
        }
        let cmd = change_cmd(ctx, &["sudo", "yum", "remove"], "-y", pick.keys());
        run_removals(ctx, vec![cmd])
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------- RPM загалом
/// Система на RPM без менеджера, яким UPM може безпечно керувати (атомарна
/// чи незнайома): тільки перегляд.
pub struct PlainRpm;

impl Source for PlainRpm {
    fn id(&self) -> &'static str {
        "rpm"
    }

    fn available(&self, ctx: &Context) -> bool {
        matches!(ctx.platform.family, Some(Family::Rpm | Family::Suse))
            && ctx.have("rpm")
            && !Dnf.available(ctx)
            && !Yum.available(ctx)
            && !Zypper.available(ctx)
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        let Some(packages) = rpm_packages(ctx) else {
            return Query::failure();
        };
        if !all {
            ctx.diagnostic(
                "rpm: тут лише перегляд, і не видно, що встановили ви, — показано все",
            );
        }
        packages
            .into_iter()
            .map(|(name, version)| Item::new(name, version))
            .collect::<Vec<_>>()
            .into()
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let mut query: Query<Update> = Query::new();
        let installed = self.inventory(ctx, true, false);
        query.failed = installed.failed;
        query.unchecked = installed
            .rows
            .into_iter()
            .map(|item| item.key)
            .filter(|key| pick.has(key))
            .collect();
        query
    }

    fn upgrade(&self, ctx: &mut Context, _pick: Pick) -> Status {
        ctx.note(
            "rpm: пакунки цієї системи UPM лише показує — оновіть їх засобами дистрибутива"
                .to_string(),
        );
        Status::Skipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    const FEDORA: &str = "ID=fedora\n";
    const CENTOS: &str = "ID=\"centos\"\nID_LIKE=\"rhel fedora\"\n";

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    #[test]
    fn dnf_is_available_only_on_a_mutable_rpm_host() {
        let tools = ["dnf", "rpm"];
        assert!(Dnf.available(&Host::with(FEDORA, &tools).ctx));
        assert!(!Dnf.available(&Host::with("ID=debian\n", &tools).ctx));
        assert!(!Dnf.available(&Host::with("ID=opensuse-leap\n", &tools).ctx));
        let mut atomic = Host::with(FEDORA, &tools);
        atomic.file("run/ostree-booted", "");
        atomic.reprobe();
        assert!(!Dnf.available(&atomic.ctx));
        assert!(!Dnf.available(&Host::with(FEDORA, &["dnf"]).ctx));
    }

    #[test]
    fn dnf_remove_uses_sudo_and_only_selected_packages() {
        let keys = picked(&["vim", "htop"]);
        for (yes, expected) in [
            (true, "sudo dnf remove -y vim htop"),
            (false, "sudo dnf remove vim htop"),
        ] {
            let mut host = Host::new(FEDORA);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            assert!(Dnf.can_remove());
            assert!(Dnf.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert_eq!(host.commands(), [expected]);
        }
        let mut host = Host::new(FEDORA);
        host.ctx.dry = true;
        assert!(Dnf.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(host.commands().is_empty());
    }

    #[test]
    fn check_upgrade_stops_at_obsoletes() {
        let listing = "Upgrades\nfirefox.x86_64  141-1  updates\n\nObsoleting packages\n\
                       newthing.x86_64 1 updates\n";
        assert_eq!(parse_check_upgrade(listing), [("firefox.x86_64", "141-1")]);
    }

    /// CentOS 7: справжній вивід yum, rpm і repoquery.
    fn yum_host() -> Host {
        let mut host = Host::new(CENTOS);
        let check = print("yum/check-update.txt");
        let installed = print("yum/rpm-all.txt");
        host.sudo()
            .stub("yum", &format!("case \"$*\" in check-update*) {check}; exit 100 ;; esac"))
            .stub(
                "rpm",
                &format!("case \"$1\" in -qa) {installed} ;; -q) echo 4.2.46-34.el7 ;; esac"),
            )
            .stub("repoquery", &print("yum/repoquery-reason.txt"));
        host
    }

    fn names(items: &[Item]) -> Vec<&str> {
        items.iter().map(|item| item.name.as_str()).collect()
    }

    #[test]
    fn yum_replaces_dnf_only_when_dnf_is_absent() {
        assert!(Yum.available(&Host::with(CENTOS, &["yum", "rpm"]).ctx));
        let both = Host::with(CENTOS, &["yum", "dnf", "rpm"]);
        assert!(!Yum.available(&both.ctx));
        assert!(Dnf.available(&both.ctx));
        assert!(!Yum.available(&Host::with("ID=debian\n", &["yum", "rpm"]).ctx));
        let mut atomic = Host::with(CENTOS, &["yum", "rpm"]);
        atomic.file("run/ostree-booted", "").reprobe();
        assert!(!Yum.available(&atomic.ctx));
    }

    #[test]
    fn yum_lists_user_installed_packages_by_default() {
        let mut host = yum_host();
        let user = Yum.inventory(&mut host.ctx, false, false);
        assert!(!user.failed);
        // tree, which і less встановлено вже після знімання списку rpm.
        assert_eq!(user.rows.len(), 11);
        assert!(names(&user.rows).contains(&"bash"));
        assert!(!names(&user.rows).contains(&"acl"));
        let all = Yum.inventory(&mut host.ctx, true, false);
        assert!(all.rows.len() > 140);
        assert!(names(&all.rows).contains(&"acl"));
    }

    #[test]
    fn yum_without_repoquery_lists_everything_and_says_so() {
        let mut host = Host::new(CENTOS);
        host.stub("yum", "exit 0")
            .stub("rpm", &print("yum/rpm-all.txt"));
        let listed = Yum.inventory(&mut host.ctx, false, false);
        assert!(listed.rows.len() > 140);
        assert!(host.messages().contains("repoquery"), "{}", host.messages());
    }

    #[test]
    fn yum_reports_updates_with_installed_versions() {
        let mut host = yum_host();
        let result = Yum.outdated(&mut host.ctx, Pick(None));
        assert!(!result.failed);
        assert_eq!(result.rows.len(), 51);
        let bash = &result.rows[0];
        assert_eq!(
            (bash.name.as_str(), bash.key.as_str()),
            ("bash", "bash")
        );
        assert_eq!(
            (bash.current.as_str(), bash.latest.as_str()),
            ("4.2.46-34.el7", "4.2.46-35.el7_9")
        );
        assert_eq!(
            host.calls()[0],
            "sudo yum check-update -q --setopt=metadata_expire=0"
        );
        let keys = picked(&["zlib"]);
        assert_eq!(Yum.outdated(&mut host.ctx, Pick(Some(&keys))).rows.len(), 1);
    }

    #[test]
    fn yum_dry_check_uses_the_cache_without_sudo() {
        let mut host = yum_host();
        host.ctx.dry = true;
        assert_eq!(Yum.outdated(&mut host.ctx, Pick(None)).rows.len(), 51);
        assert_eq!(host.calls()[0], "yum check-update -q -C");
        assert!(!host.calls().iter().any(|call| call.starts_with("sudo")));
    }

    #[test]
    fn yum_no_updates_is_not_a_failure_but_an_error_is() {
        for (code, failed) in [(0, false), (1, true)] {
            let mut host = Host::new(CENTOS);
            host.sudo().stub("yum", &format!("exit {code}"));
            let result = Yum.outdated(&mut host.ctx, Pick(None));
            assert_eq!((result.rows.len(), result.failed), (0, failed));
        }
    }

    #[test]
    fn wrapped_yum_lines_are_joined() {
        let listing = "a-very-long-package-name.x86_64\n                 1.2-3.el7   updates\n\
                       Security: kernel-1 is an installed security update\n\
                       zlib.x86_64   1.2.7-21.el7_9   updates\n\
                       Obsoleting Packages\nnew.x86_64 1 updates\n";
        assert_eq!(
            parse_yum_updates(listing),
            [
                ("a-very-long-package-name.x86_64".to_string(), "1.2-3.el7".to_string()),
                ("zlib.x86_64".to_string(), "1.2.7-21.el7_9".to_string()),
            ]
        );
    }

    #[test]
    fn yum_changes_go_through_sudo_and_confirm_only_with_yes() {
        for (yes, flag) in [(true, " -y"), (false, "")] {
            let mut host = Host::new(CENTOS);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            let keys = picked(&["tree"]);
            assert!(Yum.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert!(Yum.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Yum.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(Yum.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert_eq!(
                host.commands(),
                [
                    format!("sudo yum update{flag}"),
                    format!("sudo yum update{flag} tree"),
                    format!("sudo yum remove{flag} tree"),
                ]
            );
        }
        assert!(Yum.can_remove() && Yum.needs_sudo());
    }

    #[test]
    fn refused_removal_is_a_failure_and_is_not_forced() {
        let mut host = Host::new(CENTOS);
        host.sudo().stub("yum", "exit 1");
        host.ctx.yes = true;
        let keys = picked(&["yum"]);
        assert!(Yum.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Failed);
        assert_eq!(host.calls(), ["sudo yum remove -y yum", "yum remove -y yum"]);
    }

    #[test]
    fn generic_rpm_covers_hosts_without_a_manager_upm_can_drive() {
        assert!(PlainRpm.available(&Host::with(FEDORA, &["rpm"]).ctx));
        assert!(PlainRpm.available(&Host::with("ID=opensuse-leap\n", &["rpm"]).ctx));
        assert!(PlainRpm.available(&Host::with("ID=opensuse-microos\n", &["rpm", "zypper"]).ctx));
        let mut atomic = Host::with(FEDORA, &["rpm", "dnf"]);
        atomic.file("run/ostree-booted", "").reprobe();
        assert!(PlainRpm.available(&atomic.ctx));
        // Є кому керувати — загальне джерело не потрібне.
        for manager in ["dnf", "yum"] {
            assert!(!PlainRpm.available(&Host::with(FEDORA, &["rpm", manager]).ctx));
        }
        let leap = Host::with("ID=opensuse-leap\n", &["rpm", "zypper"]);
        assert!(!PlainRpm.available(&leap.ctx));
        assert!(!PlainRpm.available(&Host::with("ID=debian\n", &["rpm"]).ctx));
        assert!(!PlainRpm.available(&Host::with(FEDORA, &[]).ctx));
    }

    #[test]
    fn generic_rpm_only_reads() {
        let mut host = Host::new(FEDORA);
        host.stub("rpm", &print("yum/rpm-all.txt"));
        let listed = PlainRpm.inventory(&mut host.ctx, false, false);
        assert!(listed.rows.len() > 140);
        assert!(host.messages().contains("лише перегляд"), "{}", host.messages());

        let checked = PlainRpm.outdated(&mut host.ctx, Pick(None));
        assert!(checked.rows.is_empty() && !checked.failed);
        assert_eq!(checked.unchecked.len(), listed.rows.len());

        let before = host.calls().len();
        assert!(PlainRpm.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(host.ctx.notes[0].starts_with("rpm:"));
        let keys = picked(&["bash"]);
        assert!(PlainRpm.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Skipped);
        assert!(!PlainRpm.can_remove() && !PlainRpm.needs_sudo());
        assert_eq!(host.calls().len(), before);
        assert!(host.commands().is_empty());
    }
}
