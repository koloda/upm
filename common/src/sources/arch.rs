//! Arch та похідні: Pacman і AUR.

use crate::context::{Cmd, Context};
use crate::platform::Family;
use crate::source::{Item, Pick, Query, Source, Status, Update};
use crate::sources::{change_cmd, confirmable, query, run_removals};

/// Вивід запиту pacman. Код 1 без жодного тексту — це порожня відповідь.
fn read_or_none(ctx: &Context, cmd: Cmd) -> Option<String> {
    // stderr потрібен нам, щоб відрізнити порожню відповідь від помилки.
    let result = ctx.capture(&cmd.quiet());
    match result.code {
        Some(0) => Some(result.stdout),
        Some(1) if result.stdout.trim().is_empty() && result.stderr.trim().is_empty() => {
            Some(String::new())
        }
        _ => {
            ctx.diagnostic(result.stderr.trim());
            None
        }
    }
}

/// Пакунки з рядків "назва версія".
fn inventory_of(ctx: &Context, flags: &str) -> Query<Item> {
    let Some(listing) = read_or_none(ctx, query(["pacman", flags])) else {
        return Query::failure();
    };
    listing
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(name, version)| Item::new(name, version))
        .collect::<Vec<_>>()
        .into()
}

/// Рядки "назва стара -> нова"; ігноровані пакунки оновленням не вважаються.
fn parse_updates(listing: &str) -> Vec<(&str, &str, &str)> {
    listing
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let [name, current, "->", latest] = fields[..] else {
                return None;
            };
            Some((name, current, latest))
        })
        .collect()
}

fn updates_from(listing: &str, pick: Pick) -> Query<Update> {
    parse_updates(listing)
        .into_iter()
        .filter(|(name, _, _)| pick.has(name))
        .map(|(name, current, latest)| Update::new(name, current, latest))
        .collect::<Vec<_>>()
        .into()
}

/// Видалення без каскаду: pacman сам відмовить, якщо пакунок комусь потрібен.
fn remove_foreign_or_native(ctx: &mut Context, id: &str, pick: Pick) -> Status {
    if pick.keys().is_empty() || !confirmable(ctx, id) {
        return Status::Skipped;
    }
    let cmd = change_cmd(ctx, &["sudo", "pacman", "-R"], "--noconfirm", pick.keys());
    run_removals(ctx, vec![cmd])
}

// ---------------------------------------------------------------- Pacman
/// Пакунки з репозиторіїв дистрибутива.
pub struct Pacman {
    /// Незмінна система (SteamOS): тільки перегляд.
    pub read_only: bool,
}

impl Pacman {
    fn refuse_change(&self, ctx: &mut Context) -> bool {
        if self.read_only {
            ctx.note("pacman: системні пакунки тут лише для читання".to_string());
        }
        self.read_only
    }
}

impl Source for Pacman {
    fn id(&self) -> &'static str {
        "pacman"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.family == Some(Family::Arch) && ctx.have("pacman")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        // "n" — лише з репозиторіїв; сторонні належать джерелу AUR.
        inventory_of(ctx, if all { "-Qn" } else { "-Qen" })
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        // `pacman -Sy` без `-u` веде до часткового оновлення, тому свіжі дані
        // дає лише checkupdates з власною копією бази.
        if !ctx.dry && ctx.have("checkupdates") {
            let result = ctx.capture(&query(["checkupdates"]).quiet());
            // 2 — "оновлень немає".
            if !matches!(result.code, Some(0 | 2)) {
                ctx.diagnostic(result.stderr.trim());
                return Query::failure();
            }
            return updates_from(&result.stdout, pick);
        }
        ctx.diagnostic(
            "pacman: дані з кешу, можуть бути застарілими (свіжі дає checkupdates \
             з pacman-contrib поза --dry-run)",
        );
        match read_or_none(ctx, query(["pacman", "-Qun"])) {
            Some(listing) => updates_from(&listing, pick),
            None => Query::failure(),
        }
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        if self.refuse_change(ctx) {
            return Status::Skipped;
        }
        if pick.targeted() {
            ctx.note(
                "pacman: окремі пакунки не оновлюються (часткові оновлення ламають Arch) — \
                 оновіть джерело цілком"
                    .to_string(),
            );
            return Status::Skipped;
        }
        if !confirmable(ctx, self.id()) {
            return Status::Skipped;
        }
        let cmd = change_cmd(ctx, &["sudo", "pacman", "-Syu"], "--noconfirm", &[]);
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        !self.read_only
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        if self.refuse_change(ctx) {
            return Status::Skipped;
        }
        remove_foreign_or_native(ctx, self.id(), pick)
    }

    fn needs_sudo(&self) -> bool {
        !self.read_only
    }
}

// ---------------------------------------------------------------- AUR
/// Сторонні пакунки (`pacman -Qm`): переважно з AUR.
pub struct Aur;

/// AUR-помічник, якщо він є. Він сам викликає sudo, тому не запускається
/// через нього.
fn helper(ctx: &Context) -> Option<&'static str> {
    ["paru", "yay"].into_iter().find(|name| ctx.have(name))
}

impl Source for Aur {
    fn id(&self) -> &'static str {
        "aur"
    }

    fn available(&self, ctx: &Context) -> bool {
        ctx.platform.manages(Family::Arch) && ctx.have("pacman")
    }

    fn inventory(&self, ctx: &mut Context, all: bool, _versions: bool) -> Query<Item> {
        inventory_of(ctx, if all { "-Qm" } else { "-Qem" })
    }

    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update> {
        let Some(helper) = helper(ctx) else {
            ctx.diagnostic("aur: щоб перевіряти оновлення, потрібен paru або yay");
            let installed = self.inventory(ctx, true, false);
            let mut query: Query<Update> = Query::new();
            query.failed = installed.failed;
            query.unchecked = installed
                .rows
                .into_iter()
                .map(|item| item.key)
                .filter(|key| pick.has(key))
                .collect();
            return query;
        };
        let result = ctx.capture(&query([helper, "-Qua"]).quiet());
        // Код 1 без помилки — "оновлень немає"; у stderr бувають зауваження.
        let nothing = result.code == Some(1)
            && result.stdout.trim().is_empty()
            && !result.stderr.contains("error");
        if result.code != Some(0) && !nothing {
            ctx.diagnostic(result.stderr.trim());
            return Query::failure();
        }
        updates_from(&result.stdout, pick)
    }

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status {
        let Some(helper) = helper(ctx) else {
            ctx.note("aur: щоб оновлювати, потрібен paru або yay — пропущено".to_string());
            return Status::Skipped;
        };
        // Пакунки AUR збираються зі сценаріїв сторонніх людей: без термінала,
        // де їх можна переглянути, нічого не збираємо.
        if !ctx.interactive() {
            ctx.note(
                "aur: сценарії збирання треба переглянути — оновіть пакунки AUR з термінала"
                    .to_string(),
            );
            return Status::Skipped;
        }
        let action = if pick.targeted() { "-S" } else { "-Sua" };
        let cmd = change_cmd(ctx, &[helper, action], "--noconfirm", pick.keys()).interactive();
        Status::from_success(ctx.run(&cmd))
    }

    fn can_remove(&self) -> bool {
        true
    }

    fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
        remove_foreign_or_native(ctx, self.id(), pick)
    }

    fn needs_sudo(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Host, print};

    const ARCH: &str = "ID=arch\n";
    const STEAMOS: &str = include_str!("../../tests/fixtures/steamos/os-release.txt");
    const PACMAN: Pacman = Pacman { read_only: false };

    fn picked(keys: &[&str]) -> Vec<String> {
        keys.iter().map(|key| key.to_string()).collect()
    }

    /// Arch із двома оновленнями; справжній вивід.
    fn arch() -> Host {
        let mut host = Host::new(ARCH);
        let explicit = print("pacman/native-explicit.txt");
        let all = print("pacman/native-all.txt");
        let cached = print("pacman/qun-updates.txt");
        host.stub(
            "pacman",
            &format!("case $1 in -Qen) {explicit} ;; -Qn) {all} ;; -Qun) {cached} ;; esac"),
        );
        host
    }

    fn versions(updates: &[Update]) -> Vec<(&str, &str, &str)> {
        updates
            .iter()
            .map(|u| (u.name.as_str(), u.current.as_str(), u.latest.as_str()))
            .collect()
    }

    const PENDING: [(&str, &str, &str); 2] = [
        ("tree", "2.2.1-1", "2.3.2-1"),
        ("which", "2.21-6", "2.25-1"),
    ];

    #[test]
    fn pacman_is_available_on_arch_even_when_read_only() {
        assert!(PACMAN.available(&Host::with(ARCH, &["pacman"]).ctx));
        assert!(PACMAN.available(&Host::with("ID=manjaro\nID_LIKE=arch\n", &["pacman"]).ctx));
        assert!(PACMAN.available(&Host::with(STEAMOS, &["pacman"]).ctx));
        assert!(!PACMAN.available(&Host::with("ID=fedora\n", &["pacman"]).ctx));
        assert!(!PACMAN.available(&Host::with(ARCH, &[]).ctx));
    }

    #[test]
    fn pacman_lists_only_repository_packages() {
        let mut host = arch();
        let explicit = PACMAN.inventory(&mut host.ctx, false, false);
        assert_eq!(explicit.rows.len(), 5);
        let all = PACMAN.inventory(&mut host.ctx, true, false);
        assert_eq!(all.rows.len(), 141);
        assert_eq!(host.calls(), ["pacman -Qen", "pacman -Qn"]);
    }

    #[test]
    fn empty_pacman_answer_is_not_a_failure_but_an_error_is() {
        let mut host = Host::new(ARCH);
        host.stub("pacman", "exit 1");
        let listed = PACMAN.inventory(&mut host.ctx, false, false);
        assert!(listed.rows.is_empty() && !listed.failed);
        host.stub("pacman", "echo 'error: database missing' >&2; exit 1");
        assert!(PACMAN.inventory(&mut host.ctx, false, false).failed);
        assert!(PACMAN.outdated(&mut host.ctx, Pick(None)).failed);
    }

    #[test]
    fn fresh_check_uses_checkupdates_and_never_syncs_the_system() {
        let mut host = arch();
        host.stub("checkupdates", &print("pacman/checkupdates-updates.txt"));
        let result = PACMAN.outdated(&mut host.ctx, Pick(None));
        assert_eq!(versions(&result.rows), PENDING);
        assert_eq!(host.calls(), ["checkupdates "]);
        assert!(!host.messages().contains("кеш"));
        for (code, failed) in [(2, false), (1, true)] {
            host.stub("checkupdates", &format!("exit {code}"));
            let result = PACMAN.outdated(&mut host.ctx, Pick(None));
            assert_eq!((result.rows.len(), result.failed), (0, failed));
        }
    }

    #[test]
    fn without_checkupdates_or_in_dry_run_the_cache_is_used_and_reported() {
        for dry in [false, true] {
            let mut host = arch();
            host.ctx.dry = dry;
            if dry {
                host.stub("checkupdates", "exit 1");
            }
            let result = PACMAN.outdated(&mut host.ctx, Pick(None));
            assert_eq!(versions(&result.rows), PENDING);
            assert_eq!(host.calls(), ["pacman -Qun"]);
            assert!(host.messages().contains("кеш"), "{}", host.messages());
            let keys = picked(&["which"]);
            assert_eq!(PACMAN.outdated(&mut host.ctx, Pick(Some(&keys))).rows.len(), 1);
        }
    }

    #[test]
    fn ignored_packages_are_not_updates() {
        let updates = parse_updates("linux 1-1 -> 2-1 [ignored]\ntree 1 -> 2\nнесправжнє\n");
        assert_eq!(updates, [("tree", "1", "2")]);
    }

    #[test]
    fn pacman_upgrades_only_the_whole_system() {
        for (yes, flag) in [(true, " --noconfirm"), (false, "")] {
            let mut host = Host::new(ARCH);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            assert!(PACMAN.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert_eq!(host.commands(), [format!("sudo pacman -Syu{flag}")]);
        }
        let mut host = Host::new(ARCH);
        host.ctx.yes = true;
        let keys = picked(&["tree"]);
        assert!(PACMAN.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Skipped);
        assert!(host.commands().is_empty() && host.calls().is_empty());
        assert!(host.ctx.notes[0].contains("цілком"), "{:?}", host.ctx.notes);
    }

    #[test]
    fn pacman_removes_only_selected_without_cascade() {
        for (yes, flag) in [(true, " --noconfirm"), (false, "")] {
            let mut host = Host::new(ARCH);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            let keys = picked(&["tree", "which"]);
            assert!(PACMAN.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert!(PACMAN.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert_eq!(host.commands(), [format!("sudo pacman -R{flag} tree which")]);
        }
        assert!(PACMAN.can_remove() && PACMAN.needs_sudo());
    }

    #[test]
    fn pacman_does_not_pretend_when_nobody_can_confirm() {
        // pacman на кінці вводу виходить успішно, нічого не зробивши.
        let mut host = Host::new(ARCH);
        host.headless();
        let keys = picked(&["tree"]);
        assert!(PACMAN.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(PACMAN.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Skipped);
        assert!(host.commands().is_empty());
        assert_eq!(host.ctx.notes.len(), 2);
    }

    #[test]
    fn immutable_pacman_declines_changes() {
        let mut host = Host::new(STEAMOS);
        host.ctx.yes = true;
        let source = Pacman { read_only: true };
        let keys = picked(&["tree"]);
        assert!(!source.can_remove() && !source.needs_sudo());
        assert!(source.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(source.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Skipped);
        assert!(host.commands().is_empty() && host.calls().is_empty());
        assert!(host.ctx.notes[0].contains("лише"));
    }

    /// Arch зі сторонніми пакунками та вказаними AUR-помічниками.
    fn aur(helpers: &[&str]) -> Host {
        let mut host = Host::new(ARCH);
        let explicit = print("pacman/foreign-explicit.txt");
        let all = print("pacman/foreign-all.txt");
        host.stub(
            "pacman",
            &format!("case $1 in -Qem) {explicit} ;; -Qm) {all} ;; esac"),
        );
        for helper in helpers {
            let updates = print(&format!("aur/qua-{helper}.txt"));
            host.stub(helper, &updates);
        }
        host
    }

    #[test]
    fn aur_is_available_only_on_a_mutable_arch_host() {
        assert!(Aur.available(&Host::with(ARCH, &["pacman"]).ctx));
        assert!(!Aur.available(&Host::with(STEAMOS, &["pacman"]).ctx));
        assert!(!Aur.available(&Host::with("ID=fedora\n", &["pacman", "paru"]).ctx));
    }

    #[test]
    fn aur_lists_foreign_packages() {
        let mut host = aur(&[]);
        for all in [false, true] {
            let listed = Aur.inventory(&mut host.ctx, all, false);
            let names: Vec<&str> = listed.rows.iter().map(|item| item.name.as_str()).collect();
            assert_eq!(names, ["downgrade", "upm-local-only"]);
        }
        assert_eq!(host.calls(), ["pacman -Qem", "pacman -Qm"]);
    }

    #[test]
    fn paru_is_preferred_and_ignored_updates_are_hidden() {
        for (helpers, used) in [
            (&["paru", "yay"][..], "paru -Qua"),
            (&["yay"][..], "yay -Qua"),
        ] {
            let mut host = aur(helpers);
            let result = Aur.outdated(&mut host.ctx, Pick(None));
            assert_eq!(versions(&result.rows), [("downgrade", "1.0-1", "11.5.0-1")]);
            assert!(result.unchecked.is_empty() && !result.failed);
            assert_eq!(host.calls(), [used]);
        }
    }

    #[test]
    fn aur_no_updates_is_not_a_failure_but_an_error_is() {
        let mut host = aur(&["paru"]);
        for (body, failed) in [
            ("exit 1", false),
            ("echo ' -> Flagged Out Of Date AUR Packages: x' >&2; exit 1", false),
            ("echo 'error: could not reach the AUR' >&2; exit 1", true),
            ("exit 2", true),
        ] {
            host.stub("paru", body);
            let result = Aur.outdated(&mut host.ctx, Pick(None));
            assert_eq!((result.rows.len(), result.failed), (0, failed), "{body}");
        }
    }

    #[test]
    fn without_a_helper_nothing_is_called_up_to_date() {
        let mut host = aur(&[]);
        let result = Aur.outdated(&mut host.ctx, Pick(None));
        assert!(result.rows.is_empty() && !result.failed);
        assert_eq!(result.unchecked, ["downgrade", "upm-local-only"]);
        assert!(host.messages().contains("paru"));
        assert!(Aur.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert!(host.ctx.notes[0].contains("paru"));
        assert!(host.commands().is_empty());
    }

    #[test]
    fn aur_builds_nothing_without_a_terminal() {
        for yes in [false, true] {
            let mut host = aur(&["paru"]);
            host.headless();
            host.ctx.yes = yes;
            assert!(Aur.upgrade(&mut host.ctx, Pick(None)) == Status::Skipped);
            assert!(host.commands().is_empty() && host.calls().is_empty());
            assert!(host.ctx.notes[0].contains("термінал"), "{:?}", host.ctx.notes);
        }
    }

    #[test]
    fn aur_upgrades_through_the_helper_and_confirms_only_with_yes() {
        for (yes, flag) in [(true, " --noconfirm"), (false, "")] {
            let mut host = aur(&["paru"]);
            host.ctx.dry = true;
            host.ctx.yes = yes;
            let keys = picked(&["downgrade"]);
            assert!(Aur.upgrade(&mut host.ctx, Pick(None)) == Status::Ok);
            assert!(Aur.upgrade(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
            assert_eq!(
                host.commands(),
                [format!("paru -Sua{flag}"), format!("paru -S{flag} downgrade")]
            );
        }
    }

    #[test]
    fn aur_removes_through_pacman() {
        let mut host = aur(&[]);
        host.ctx.dry = true;
        let keys = picked(&["upm-local-only"]);
        assert!(Aur.can_remove() && Aur.needs_sudo());
        assert!(Aur.remove(&mut host.ctx, Pick(Some(&keys))) == Status::Ok);
        assert!(Aur.remove(&mut host.ctx, Pick(None)) == Status::Skipped);
        assert_eq!(host.commands(), ["sudo pacman -R upm-local-only"]);
    }
}
