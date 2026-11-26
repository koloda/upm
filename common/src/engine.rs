//! Операції list, update та upgrade. Повертають дані; як їх показати,
//! вирішує інтерфейс.

use std::process::Stdio;
use std::time::Duration;

use crate::context::{Cmd, Context};
use crate::scope::{Picks, Scope, Target, pick};
use crate::source::{Item, Source, Status, Update};

type Sources = [Box<dyn Source>];

/// Чи назвав користувач це джерело: тоді про його відсутність треба сказати.
/// Решту відсутніх тут джерел операції мовчки пропускають.
fn asked_for(scope: &Scope, picks: &Picks, id: &str) -> bool {
    scope.named(id) || scope.only.iter().any(|only| *only == id) || picks.contains_key(id)
}

// ---------------------------------------------------------------- Пошук названих пакунків
#[derive(Clone, Copy)]
enum Match {
    Exact,
    /// Скорочена назва: "codex" для "@openai/codex".
    Alias,
}

fn matches(item: &Item, query: &str, mode: Match) -> bool {
    match mode {
        Match::Exact => item.name.to_lowercase() == query,
        Match::Alias => item.aliases.iter().any(|a| a.to_lowercase() == query),
    }
}

/// Перетворює названі пакунки на ключі. Якщо хоч одного не знайдено,
/// повертає перелік незнайдених — тоді не слід робити нічого.
pub fn resolve(ctx: &mut Context, scope: &Scope, sources: &Sources) -> Result<Picks, Vec<String>> {
    let mut cache: std::collections::HashMap<&str, Vec<Item>> = Default::default();
    let mut picks = Picks::new();
    let mut missing = Vec::new();

    for Target {
        source: wanted,
        query,
    } in &scope.targets
    {
        let lower = query.to_lowercase();
        let mut found = false;
        // Спершу точні назви всюди; скорочені — лише якщо точних немає.
        for mode in [Match::Exact, Match::Alias] {
            for source in sources {
                let id = source.id();
                let searched = scope.in_scope(id)
                    && match wanted {
                        Some(wanted) => *wanted == id,
                        None => scope.allows(id),
                    };
                if !searched || !source.available(ctx) {
                    continue;
                }
                let items = cache
                    .entry(id)
                    .or_insert_with(|| source.inventory(ctx, true, false).rows);
                for item in items.iter().filter(|item| matches(item, &lower, mode)) {
                    found = true;
                    ctx.info(&format!("знайдено: {id}:{}", item.name));
                    let keys = picks.entry(id).or_default();
                    if !keys.contains(&item.key) {
                        keys.push(item.key.clone());
                    }
                }
            }
            if found {
                break;
            }
        }
        if !found {
            missing.push(match wanted {
                Some(source) => format!("{source}:{query}"),
                None => query.clone(),
            });
        }
    }

    if missing.is_empty() {
        Ok(picks)
    } else {
        Err(missing)
    }
}

// ---------------------------------------------------------------- list
/// Установлене в одному джерелі.
pub struct Listing {
    pub source: &'static str,
    /// Відфільтровані й відсортовані за назвою. Порожньо буває лише
    /// для явно вибраного джерела.
    pub items: Vec<Item>,
    /// Список отримати не вдалося або він неповний.
    pub failed: bool,
}

pub struct ListOutcome {
    /// Запити, за якими нічого не знайдено.
    pub missing: Vec<String>,
    pub failed: bool,
    /// У списку є й залежності.
    pub includes_dependencies: bool,
    pub interrupted: bool,
}

fn contains(item: &Item, query: &str) -> bool {
    std::iter::once(&item.name)
        .chain(&item.aliases)
        .any(|name| name.to_lowercase().contains(query))
}

/// Передає в `on_listing` джерела по одному, щойно кожне готове.
/// Названі пакунки тут шукаються за частиною назви.
pub fn list(
    ctx: &mut Context,
    scope: &Scope,
    all: bool,
    sources: &Sources,
    mut on_listing: impl FnMut(&mut Context, Listing),
) -> ListOutcome {
    // Пошук за назвою йде й по залежностях.
    let all = all || !scope.targets.is_empty();
    let mut found = vec![false; scope.targets.len()];
    let mut outcome = ListOutcome {
        missing: Vec::new(),
        failed: false,
        includes_dependencies: all,
        interrupted: false,
    };

    for source in sources {
        let id = source.id();
        if !scope.in_scope(id) {
            continue;
        }
        let queries: Vec<(usize, String)> = scope
            .targets
            .iter()
            .enumerate()
            .filter(|(_, target)| match target.source {
                Some(wanted) => wanted == id,
                None => scope.allows(id),
            })
            .map(|(index, target)| (index, target.query.to_lowercase()))
            .collect();
        let wanted = if scope.targets.is_empty() {
            scope.allows(id)
        } else {
            !queries.is_empty() || (!scope.has_bare_target() && scope.named(id))
        };
        if !wanted || !source.available(ctx) {
            continue;
        }

        let result = source.inventory(ctx, all, true);
        if ctx.interrupted() {
            outcome.interrupted = true;
            return outcome;
        }
        outcome.failed |= result.failed;
        let mut items = result.rows;
        if !queries.is_empty() {
            items.retain(|item| {
                let mut keep = false;
                for (index, query) in &queries {
                    if contains(item, query) {
                        found[*index] = true;
                        keep = true;
                    }
                }
                keep
            });
        }
        items.sort_by_key(|item| item.name.to_lowercase());
        let show_empty = queries.is_empty() && scope.named(id);
        if !items.is_empty() || show_empty || result.failed {
            on_listing(
                ctx,
                Listing {
                    source: id,
                    items,
                    failed: result.failed,
                },
            );
        }
    }

    outcome.missing = scope
        .targets
        .iter()
        .zip(found)
        .filter(|(_, found)| !found)
        .map(|(target, _)| target.query.clone())
        .collect();
    outcome
}

// ---------------------------------------------------------------- update
/// Результат перевірки оновлень одного джерела.
pub struct SourceCheck {
    pub source: &'static str,
    /// Чи є це джерело на комп'ютері.
    pub available: bool,
    pub updates: Vec<Update>,
    pub failed: bool,
    /// Ключі пакунків, стан яких з'ясувати не вдалося.
    pub unchecked: Vec<String>,
    /// Вибрані пакунки, для яких оновлень не знайдено (їхні назви).
    pub without_updates: Vec<String>,
    /// Чи були вибрані конкретні пакунки.
    pub targeted: bool,
}

pub enum CheckEvent {
    Started(&'static str),
    Finished(SourceCheck),
}

/// Перевіряє оновлення, нічого не встановлюючи.
/// Повертає `false`, якщо роботу перервано.
pub fn check_updates(
    ctx: &mut Context,
    scope: &Scope,
    picks: &Picks,
    sources: &Sources,
    mut on_event: impl FnMut(&mut Context, CheckEvent),
) -> bool {
    for source in sources {
        let id = source.id();
        // Те, що оновлюється вручну, перевіряти нема чим — хіба що його назвали.
        if !scope.is_active(picks, id) || (id == "manual" && !picks.contains_key(id)) {
            continue;
        }
        let available = source.available(ctx);
        if !available && !asked_for(scope, picks, id) {
            continue;
        }
        on_event(ctx, CheckEvent::Started(id));
        let selection = pick(picks, id);
        let mut check = SourceCheck {
            source: id,
            available,
            updates: Vec::new(),
            failed: false,
            unchecked: Vec::new(),
            without_updates: Vec::new(),
            targeted: selection.targeted(),
        };
        if check.available {
            let result = source.outdated(ctx, selection);
            if ctx.interrupted() {
                return false;
            }
            check.failed = result.failed;
            check.updates = result.rows;
            check.unchecked = result.unchecked;
            if id != "manual" {
                check.without_updates = selection
                    .keys()
                    .iter()
                    .filter(|key| !check.updates.iter().any(|update| update.key == **key))
                    .map(|key| {
                        let name = key.split_once(':').map_or(key.as_str(), |(_, name)| name);
                        name.to_string()
                    })
                    .collect();
            }
        }
        on_event(ctx, CheckEvent::Finished(check));
    }
    true
}

// ---------------------------------------------------------------- upgrade
#[derive(Default)]
pub struct Summary {
    pub ok: Vec<&'static str>,
    pub skipped: Vec<&'static str>,
    pub failed: Vec<&'static str>,
    pub interrupted: bool,
}

/// Пароль sudo один раз на початку, далі підтримуємо його активним.
fn keep_sudo(ctx: &Context) {
    if !ctx.capture(&Cmd::new(["sudo", "-v"]).interactive()).ok() {
        ctx.warn("Без sudo частина оновлень завершиться з помилкою");
        return;
    }
    // Потік зникає разом із програмою.
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_secs(50));
            let alive = std::process::Command::new("sudo")
                .args(["-n", "-v"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if !alive {
                break;
            }
        }
    });
}

/// Оновлює вибране. `on_start` викликається перед кожним джерелом.
pub fn upgrade(
    ctx: &mut Context,
    scope: &Scope,
    picks: &Picks,
    sources: &Sources,
    mut on_start: impl FnMut(&mut Context, &'static str),
) -> Summary {
    // Для окремих пакунків sudo сам запитає пароль, якщо він справді потрібен.
    let needs_sudo = sources.iter().any(|source| {
        source.needs_sudo() && scope.is_active(picks, source.id()) && source.available(ctx)
    });
    if !ctx.dry && scope.targets.is_empty() && picks.is_empty() && needs_sudo {
        keep_sudo(ctx);
    }

    let mut summary = Summary::default();
    for source in sources {
        let id = source.id();
        if ctx.interrupted() {
            break;
        }
        if !scope.is_active(picks, id) || (id == "manual" && !picks.contains_key(id)) {
            continue;
        }
        let available = source.available(ctx);
        if !available && !asked_for(scope, picks, id) {
            continue;
        }
        on_start(ctx, id);
        let status = if available {
            source.upgrade(ctx, pick(picks, id))
        } else {
            Status::Skipped
        };
        match status {
            Status::Ok => summary.ok.push(id),
            Status::Skipped => summary.skipped.push(id),
            Status::Failed => summary.failed.push(id),
        }
    }
    summary.interrupted = ctx.interrupted();
    summary
}

// ---------------------------------------------------------------- видалення
/// Видаляє тільки явно вибране. На відміну від `upgrade`, порожній вибір
/// ніколи не означає «все»: джерела без власної команди видалення
/// пропускаються, а не видаляють файли самі.
pub fn remove(
    ctx: &mut Context,
    scope: &Scope,
    picks: &Picks,
    sources: &Sources,
    mut on_start: impl FnMut(&mut Context, &'static str),
) -> Summary {
    if picks.is_empty() {
        return Summary::default();
    }
    let needs_sudo = sources.iter().any(|source| {
        source.can_remove()
            && source.needs_sudo()
            && scope.is_active(picks, source.id())
            && picks
                .get(source.id())
                .is_some_and(|keys| !keys.is_empty())
            && source.available(ctx)
    });
    if !ctx.dry && needs_sudo {
        keep_sudo(ctx);
    }

    let mut summary = Summary::default();
    for source in sources {
        let id = source.id();
        if ctx.interrupted() {
            break;
        }
        // Видалення завжди цільове: лише ключі, що є в плані.
        if !picks.get(id).is_some_and(|keys| !keys.is_empty())
            || !scope.is_active(picks, id)
            || !source.can_remove()
        {
            continue;
        }
        on_start(ctx, id);
        let status = if source.available(ctx) {
            source.remove(ctx, pick(picks, id))
        } else {
            Status::Skipped
        };
        match status {
            Status::Ok => summary.ok.push(id),
            Status::Skipped => summary.skipped.push(id),
            Status::Failed => summary.failed.push(id),
        }
    }
    summary.interrupted = ctx.interrupted();
    summary
}

// ---------------------------------------------------------------- Ручне оновлення
/// Те, що не оновлюється автоматично.
pub struct ManualReport {
    /// RPM-пакунки поза увімкненими репозиторіями: назва й версія.
    pub orphan_rpms: Vec<(String, String)>,
    pub applications: Vec<Item>,
}

pub fn manual_report(ctx: &mut Context, sources: &Sources) -> ManualReport {
    let mut orphan_rpms = Vec::new();
    let has_dnf = sources
        .iter()
        .any(|source| source.id() == "dnf" && source.available(ctx));
    if has_dnf {
        let query = Cmd::new(["dnf", "--cacheonly", "--quiet", "list", "--extras"]).quiet();
        orphan_rpms = ctx
            .capture(&query)
            .stdout
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let (name, version) = (words.next()?, words.next()?);
                (name.contains('.') && !name.starts_with("kmod"))
                    .then(|| (name.to_string(), version.to_string()))
            })
            .collect();
    }
    let applications = sources
        .iter()
        .find(|source| source.id() == "manual")
        .map(|manual| manual.inventory(ctx, false, false).rows)
        .unwrap_or_default();
    ManualReport {
        orphan_rpms,
        applications,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::report::Reporter;
    use crate::source::{Pick, Query};
    use crate::testing::Host;

    #[derive(Clone, Default)]
    struct Recorder(Arc<Mutex<Vec<String>>>);

    impl Reporter for Recorder {
        fn info(&self, text: &str) {
            self.0.lock().unwrap().push(text.to_string());
        }
        fn warn(&self, _: &str) {}
        fn diagnostic(&self, _: &str) {}
        fn log(&self, _: &str) {}
        fn command(&self, _: &str, _: bool) {}
        fn failure(&self, _: &str) {}
    }

    /// Джерело з двома пакунками, одне оновлення.
    struct Fake;

    impl Source for Fake {
        fn id(&self) -> &'static str {
            "npm"
        }
        fn available(&self, _: &Context) -> bool {
            true
        }
        fn inventory(&self, _: &mut Context, _: bool, _: bool) -> Query<Item> {
            vec![
                Item::new("zeta", "1.0"),
                Item::new("@scope/alpha", "2.0").alias("alpha"),
            ]
            .into()
        }
        fn outdated(&self, _: &mut Context, _: Pick) -> Query<Update> {
            vec![Update::new("zeta", "1.0", "1.1")].into()
        }
        fn upgrade(&self, _: &mut Context, _: Pick) -> Status {
            Status::Ok
        }
    }

    fn setup(targets: &[&str]) -> (Context, Scope, Vec<Box<dyn Source>>, Recorder) {
        let recorder = Recorder::default();
        let ctx = Context::new("/nonexistent".into(), true, false, Box::new(recorder.clone()));
        let scope = Scope {
            targets: targets
                .iter()
                .map(|query| Target {
                    source: None,
                    query: query.to_string(),
                })
                .collect(),
            ..Scope::default()
        };
        (ctx, scope, vec![Box::new(Fake)], recorder)
    }

    #[test]
    fn list_returns_sorted_data() {
        let (mut ctx, scope, sources, _) = setup(&[]);
        let mut names = Vec::new();
        list(&mut ctx, &scope, false, &sources, |_, listing| {
            names = listing.items.into_iter().map(|item| item.name).collect();
        });
        assert_eq!(names, ["@scope/alpha", "zeta"]);
    }

    #[test]
    fn short_name_resolves_and_is_reported() {
        let (mut ctx, scope, sources, recorder) = setup(&["Alpha"]);
        let picks = resolve(&mut ctx, &scope, &sources).unwrap();
        assert_eq!(picks["npm"], ["@scope/alpha"]);
        assert_eq!(*recorder.0.lock().unwrap(), ["знайдено: npm:@scope/alpha"]);
    }

    #[test]
    fn one_unknown_name_fails_the_whole_request() {
        let (mut ctx, scope, sources, _) = setup(&["zeta", "nope"]);
        assert_eq!(resolve(&mut ctx, &scope, &sources).unwrap_err(), ["nope"]);
    }

    #[test]
    fn check_lists_picked_packages_without_updates() {
        let (mut ctx, scope, sources, _) = setup(&["zeta", "alpha"]);
        let picks = resolve(&mut ctx, &scope, &sources).unwrap();
        let mut checks = Vec::new();
        check_updates(&mut ctx, &scope, &picks, &sources, |_, event| {
            if let CheckEvent::Finished(check) = event {
                checks.push(check);
            }
        });
        assert_eq!(checks[0].updates.len(), 1);
        assert_eq!(checks[0].without_updates, ["@scope/alpha"]);
    }

    #[test]
    fn picks_without_targets_touch_only_picked_sources() {
        // Так інтерфейс оновлює позначене: готові ключі без пошуку за назвою.
        let (mut ctx, scope, mut sources, _) = setup(&[]);
        sources.push(Box::new(Other));
        let picks = Picks::from([("npm", vec!["zeta".to_string()])]);
        let mut started = Vec::new();
        upgrade(&mut ctx, &scope, &picks, &sources, |_, id| started.push(id));
        assert_eq!(started, ["npm"]);
    }

    #[test]
    fn check_passes_unchecked_keys_through() {
        let (mut ctx, scope, _, _) = setup(&[]);
        let sources: Vec<Box<dyn Source>> = vec![Box::new(Other)];
        let mut unchecked = Vec::new();
        check_updates(&mut ctx, &scope, &Picks::new(), &sources, |_, event| {
            if let CheckEvent::Finished(check) = event {
                unchecked = check.unchecked;
            }
        });
        assert_eq!(unchecked, ["mystery"]);
    }

    /// Джерело, що не вміє перевірити свій пакунок.
    struct Other;

    impl Source for Other {
        fn id(&self) -> &'static str {
            "go"
        }
        fn available(&self, _: &Context) -> bool {
            true
        }
        fn inventory(&self, _: &mut Context, _: bool, _: bool) -> Query<Item> {
            Query::new()
        }
        fn outdated(&self, _: &mut Context, _: Pick) -> Query<Update> {
            let mut query = Query::new();
            query.unchecked.push("mystery".to_string());
            query
        }
        fn upgrade(&self, _: &mut Context, _: Pick) -> Status {
            Status::Ok
        }
    }

    /// Джерело, яке є або нема, і хоче sudo.
    struct Presence(&'static str, bool);

    impl Source for Presence {
        fn id(&self) -> &'static str {
            self.0
        }
        fn available(&self, _: &Context) -> bool {
            self.1
        }
        fn inventory(&self, _: &mut Context, _: bool, _: bool) -> Query<Item> {
            Query::new()
        }
        fn outdated(&self, _: &mut Context, _: Pick) -> Query<Update> {
            Query::new()
        }
        fn upgrade(&self, _: &mut Context, _: Pick) -> Status {
            Status::Ok
        }
        fn needs_sudo(&self) -> bool {
            true
        }
    }

    fn present_and_absent() -> Vec<Box<dyn Source>> {
        vec![
            Box::new(Presence("apt", true)),
            Box::new(Presence("dnf", false)),
        ]
    }

    fn checked_sources(ctx: &mut Context, scope: &Scope) -> Vec<(&'static str, bool)> {
        let mut seen = Vec::new();
        check_updates(ctx, scope, &Picks::new(), &present_and_absent(), |_, event| {
            match event {
                CheckEvent::Started(id) => seen.push((id, false)),
                CheckEvent::Finished(check) => {
                    seen.last_mut().unwrap().1 = check.available;
                }
            }
        });
        seen
    }

    #[test]
    fn absent_source_is_silent_unless_named() {
        let (mut ctx, mut scope, _, _) = setup(&[]);
        assert_eq!(checked_sources(&mut ctx, &scope), [("apt", true)]);
        scope.sources = vec!["dnf"];
        assert_eq!(checked_sources(&mut ctx, &scope), [("dnf", false)]);
        scope = Scope {
            only: vec!["dnf"],
            ..Scope::default()
        };
        assert_eq!(checked_sources(&mut ctx, &scope), [("dnf", false)]);
    }

    #[test]
    fn upgrade_summary_omits_absent_sources_unless_named() {
        let (mut ctx, scope, _, _) = setup(&[]);
        let mut started = Vec::new();
        let summary = upgrade(&mut ctx, &scope, &Picks::new(), &present_and_absent(), |_, id| {
            started.push(id)
        });
        assert_eq!(started, ["apt"]);
        assert_eq!(summary.ok, ["apt"]);
        assert!(summary.skipped.is_empty());

        let named = Scope {
            sources: vec!["dnf"],
            ..Scope::default()
        };
        let summary = upgrade(&mut ctx, &named, &Picks::new(), &present_and_absent(), |_, _| {});
        assert_eq!(summary.skipped, ["dnf"]);
    }

    #[test]
    fn sudo_is_asked_only_for_present_sources() {
        for (present, expected) in [(false, vec![]), (true, vec!["sudo -v"])] {
            let mut host = Host::new("");
            host.stub("sudo", "exit 0");
            let sources: Vec<Box<dyn Source>> = vec![Box::new(Presence("apt", present))];
            upgrade(&mut host.ctx, &Scope::default(), &Picks::new(), &sources, |_, _| {});
            assert_eq!(host.calls(), expected, "present: {present}");
        }
    }

    #[test]
    fn orphan_rpms_are_reported_only_where_dnf_is_the_native_source() {
        let listing = "echo 'stray.x86_64 1.0 @System'";
        for (present, orphans) in [(true, 1), (false, 0)] {
            let mut host = Host::new("");
            host.stub("dnf", listing);
            let sources: Vec<Box<dyn Source>> = vec![Box::new(Presence("dnf", present))];
            let report = manual_report(&mut host.ctx, &sources);
            assert_eq!(report.orphan_rpms.len(), orphans);
            assert_eq!(host.calls().len(), orphans);
        }
    }

    #[test]
    fn cancelled_check_stops() {
        let (mut ctx, scope, sources, _) = setup(&[]);
        ctx.cancel_flag()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(!check_updates(
            &mut ctx,
            &scope,
            &Picks::new(),
            &sources,
            |_, _| {}
        ));
    }

    /// Джерело з власною командою видалення; знає лише ключ `one`.
    struct Removable(&'static str, Arc<Mutex<Vec<String>>>, Status);

    impl Source for Removable {
        fn id(&self) -> &'static str {
            self.0
        }
        fn available(&self, _: &Context) -> bool {
            true
        }
        fn inventory(&self, _: &mut Context, _: bool, _: bool) -> Query<Item> {
            vec![Item::new("one", "1")].into()
        }
        fn outdated(&self, _: &mut Context, _: Pick) -> Query<Update> {
            Query::new()
        }
        fn upgrade(&self, _: &mut Context, _: Pick) -> Status {
            Status::Ok
        }
        fn can_remove(&self) -> bool {
            true
        }
        fn remove(&self, ctx: &mut Context, pick: Pick) -> Status {
            for key in pick.keys() {
                if key == "one" {
                    self.1.lock().unwrap().push(format!("{}:{key}", self.0));
                    // dry-run: команда лише показується, не виконується.
                    ctx.run(&Cmd::new(["uninstall-stub", key]));
                }
            }
            self.2
        }
    }

    fn removing(
        status: Status,
    ) -> (Context, Scope, Vec<Box<dyn Source>>, Arc<Mutex<Vec<String>>>) {
        let (ctx, scope, _, _) = setup(&[]);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let sources: Vec<Box<dyn Source>> = vec![Box::new(Removable("npm", calls.clone(), status))];
        (ctx, scope, sources, calls)
    }

    #[test]
    fn remove_touches_only_selected_keys() {
        let (mut ctx, scope, sources, calls) = removing(Status::Ok);
        let picks = Picks::from([("npm", vec!["one".to_string()])]);
        let summary = remove(&mut ctx, &scope, &picks, &sources, |_, _| {});
        assert_eq!(*calls.lock().unwrap(), ["npm:one"]);
        assert_eq!(summary.ok, ["npm"]);
        assert!(!summary.interrupted);
    }

    #[test]
    fn remove_reports_skipped_and_failed() {
        for (status, field) in [(Status::Skipped, 0), (Status::Failed, 1)] {
            let (mut ctx, scope, sources, _) = removing(status);
            let picks = Picks::from([("npm", vec!["one".to_string()])]);
            let summary = remove(&mut ctx, &scope, &picks, &sources, |_, _| {});
            let counts = [summary.skipped.len(), summary.failed.len()];
            assert_eq!(counts[field], 1);
        }
    }

    #[test]
    fn stale_or_empty_selection_runs_no_commands() {
        let (mut ctx, scope, sources, calls) = removing(Status::Ok);
        let summary = remove(&mut ctx, &scope, &Picks::new(), &sources, |_, _| {});
        assert!(summary.ok.is_empty() && summary.failed.is_empty());
        let stale = Picks::from([("npm", vec!["gone".to_string()])]);
        remove(&mut ctx, &scope, &stale, &sources, |_, _| {});
        assert!(calls.lock().unwrap().is_empty());
    }

    #[test]
    fn sources_without_removal_are_never_called() {
        let (mut ctx, scope, _, _) = setup(&[]);
        let sources: Vec<Box<dyn Source>> = vec![Box::new(Other)];
        let picks = Picks::from([("go", vec!["one".to_string()])]);
        let summary = remove(&mut ctx, &scope, &picks, &sources, |_, _| {});
        assert!(summary.ok.is_empty() && summary.failed.is_empty());
        assert!(ctx.failed_commands.is_empty());
    }

    #[test]
    fn cancelled_removal_stops_before_the_first_item() {
        let (mut ctx, scope, sources, calls) = removing(Status::Ok);
        ctx.cancel_flag()
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let picks = Picks::from([("npm", vec!["one".to_string()])]);
        let summary = remove(&mut ctx, &scope, &picks, &sources, |_, _| {});
        assert!(summary.interrupted);
        assert!(calls.lock().unwrap().is_empty());
    }
}
