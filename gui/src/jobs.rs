//! Довгі операції, які виконуються у фоновому потоці. Qt тут немає.

use std::sync::{Mutex, MutexGuard};

use upm_common::catalog::Catalog;
use upm_common::context::Context;
use upm_common::engine::{self, CheckEvent, Summary};
use upm_common::scope::{Picks, Scope};
use upm_common::source::Source;
use upm_common::sources;

use crate::view;

pub enum Job {
    /// Прочитати встановлене.
    Load,
    /// Перевірити оновлення у вибраному джерелі або всюди.
    Check { source: Option<String> },
    Upgrade {
        source: Option<String>,
        selected: Vec<(String, String)>,
    },
}

pub struct Outcome {
    pub status: String,
    /// Як після роботи перемкнути фільтр "лише з оновленнями".
    pub only_upgradable: Option<bool>,
}

/// Каталог лишається придатним, навіть якщо інший потік впав із ним у руках.
pub fn lock(catalog: &Mutex<Catalog>) -> MutexGuard<'_, Catalog> {
    catalog.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// `changed` викликається щоразу, коли в каталозі з'явилося щось нове.
pub fn run(
    job: Job,
    ctx: &mut Context,
    sources: &[Box<dyn Source>],
    catalog: &Mutex<Catalog>,
    changed: &dyn Fn(),
) -> Outcome {
    match job {
        Job::Load => {
            load(ctx, sources::NAMES.to_vec(), sources, catalog, changed);
            Outcome {
                status: "Натисніть Refresh, щоб перевірити оновлення".to_string(),
                only_upgradable: None,
            }
        }
        Job::Check { source } => check(ctx, source.as_deref(), sources, catalog, changed),
        Job::Upgrade { source, selected } => {
            let plan = lock(catalog).upgrade_plan(source.as_deref(), &selected);
            if plan.is_empty() {
                return Outcome {
                    status: "Нічого оновлювати".to_string(),
                    only_upgradable: None,
                };
            }
            let summary = engine::upgrade(ctx, &plan.scope, &plan.picks, sources, |ctx, id| {
                ctx.log(&format!("\n==> {}\n", view::title(id)));
                changed();
            });
            let status = report(ctx, &summary);
            // Перечитуємо лише те, що чіпали.
            let touched = [summary.ok, summary.failed].concat();
            if !touched.is_empty() {
                load(ctx, touched, sources, catalog, changed);
            }
            Outcome {
                status,
                only_upgradable: Some(false),
            }
        }
    }
}

fn load(
    ctx: &mut Context,
    ids: Vec<&'static str>,
    sources: &[Box<dyn Source>],
    catalog: &Mutex<Catalog>,
    changed: &dyn Fn(),
) {
    // Названі джерела повертаються навіть порожніми — так каталог очиститься.
    let scope = Scope {
        sources: ids,
        firmware: true,
        ..Scope::default()
    };
    engine::list(ctx, &scope, false, sources, |_, listing| {
        lock(catalog).set_items(listing.source, listing.items);
        changed();
    });
}

fn check(
    ctx: &mut Context,
    source: Option<&str>,
    sources: &[Box<dyn Source>],
    catalog: &Mutex<Catalog>,
    changed: &dyn Fn(),
) -> Outcome {
    let mut scope = Scope::default();
    if let Some(id) = source.and_then(sources::canonical) {
        scope.sources.push(id);
        scope.firmware = id == "firmware";
    }
    let mut total = 0;
    let mut failed = Vec::new();
    engine::check_updates(ctx, &scope, &Picks::new(), sources, |ctx, event| match event {
        CheckEvent::Started(id) => ctx.log(&format!("\n==> {}\n", view::title(id))),
        CheckEvent::Finished(check) => {
            total += check.updates.len();
            if check.failed {
                failed.push(view::title(check.source).to_string());
            }
            if check.available {
                lock(catalog).set_check(check);
            }
            changed();
        }
    });
    let mut status = format!("Можна оновити: {total}");
    if !failed.is_empty() {
        status.push_str(&format!(". Не вдалося перевірити: {}", failed.join(", ")));
    }
    Outcome {
        status,
        only_upgradable: Some(true),
    }
}

/// Пише підсумок у журнал і повертає короткий статус.
fn report(ctx: &mut Context, summary: &Summary) -> String {
    for command in &ctx.failed_commands {
        ctx.log(&format!("✗ {command}\n"));
    }
    for note in &ctx.notes {
        ctx.log(&format!("• {note}\n"));
    }
    if summary.failed.is_empty() {
        "Оновлення завершено".to_string()
    } else {
        let failed: Vec<&str> = summary.failed.iter().map(|id| view::title(id)).collect();
        format!(
            "Оновлення з помилками: {}. Деталі в журналі",
            failed.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use upm_common::catalog::Filter;
    use upm_common::context::Context;
    use upm_common::report::Reporter;
    use upm_common::source::{Item, Pick, Query, Source, Status, Update};

    use super::*;

    #[derive(Default)]
    struct Log(std::sync::Arc<Mutex<String>>);

    impl Reporter for Log {
        fn info(&self, text: &str) {
            self.0.lock().unwrap().push_str(text);
        }
        fn warn(&self, _: &str) {}
        fn diagnostic(&self, _: &str) {}
        fn log(&self, text: &str) {
            self.0.lock().unwrap().push_str(text);
        }
        fn command(&self, line: &str, _: bool) {
            self.0.lock().unwrap().push_str(line);
        }
        fn failure(&self, _: &str) {}
    }

    /// Джерело, яке записує, що саме його просили оновити.
    struct Fake(&'static str, std::sync::Arc<Mutex<Vec<String>>>);

    impl Source for Fake {
        fn id(&self) -> &'static str {
            self.0
        }
        fn available(&self, _: &Context) -> bool {
            true
        }
        fn inventory(&self, _: &mut Context, _: bool, _: bool) -> Query<Item> {
            vec![Item::new("one", "1"), Item::new("two", "1")].into()
        }
        fn outdated(&self, _: &mut Context, _: Pick) -> Query<Update> {
            vec![Update::new("one", "1", "2")].into()
        }
        fn upgrade(&self, _: &mut Context, pick: Pick) -> Status {
            self.1
                .lock()
                .unwrap()
                .push(format!("{}:{}", self.0, pick.keys().join(",")));
            Status::Ok
        }
    }

    struct Fixture {
        ctx: Context,
        sources: Vec<Box<dyn Source>>,
        catalog: Mutex<Catalog>,
        upgraded: std::sync::Arc<Mutex<Vec<String>>>,
    }

    impl Fixture {
        fn new() -> Self {
            let upgraded = std::sync::Arc::new(Mutex::new(Vec::new()));
            Fixture {
                // dry = true: жодного sudo.
                ctx: Context::new("/nonexistent".into(), true, true, Box::new(Log::default())),
                sources: vec![
                    Box::new(Fake("npm", upgraded.clone())),
                    Box::new(Fake("go", upgraded.clone())),
                ],
                catalog: Mutex::new(Catalog::default()),
                upgraded,
            }
        }

        fn run(&mut self, job: Job) -> Outcome {
            run(job, &mut self.ctx, &self.sources, &self.catalog, &|| {})
        }

        fn upgradable(&self) -> Vec<&'static str> {
            let filter = Filter {
                source: None,
                only_upgradable: true,
                search: "",
            };
            let rows = self.catalog.lock().unwrap().rows(&filter);
            rows.iter().map(|row| row.source).collect()
        }
    }

    #[test]
    fn load_fills_the_catalog() {
        let mut fixture = Fixture::new();
        let outcome = fixture.run(Job::Load);
        assert_eq!(fixture.catalog.lock().unwrap().summaries().len(), 2);
        assert_eq!(outcome.only_upgradable, None);
    }

    #[test]
    fn check_of_one_source_leaves_others_alone() {
        let mut fixture = Fixture::new();
        fixture.run(Job::Load);
        let outcome = fixture.run(Job::Check {
            source: Some("go".into()),
        });
        assert_eq!(fixture.upgradable(), ["go"]);
        assert_eq!(outcome.only_upgradable, Some(true));
        assert!(outcome.status.contains('1'), "{}", outcome.status);
    }

    #[test]
    fn upgrade_of_selection_reloads_only_touched_sources() {
        let mut fixture = Fixture::new();
        fixture.run(Job::Load);
        fixture.run(Job::Check { source: None });
        let outcome = fixture.run(Job::Upgrade {
            source: None,
            selected: vec![("npm".into(), "one".into())],
        });
        assert_eq!(*fixture.upgraded.lock().unwrap(), ["npm:one"]);
        // npm перечитано (стан невідомий), результат перевірки go зберігся.
        assert_eq!(fixture.upgradable(), ["go"]);
        assert_eq!(outcome.only_upgradable, Some(false));
    }

    #[test]
    fn stale_selection_upgrades_nothing() {
        let mut fixture = Fixture::new();
        fixture.run(Job::Load);
        fixture.run(Job::Upgrade {
            source: None,
            selected: vec![("npm".into(), "gone".into())],
        });
        assert!(fixture.upgraded.lock().unwrap().is_empty());
    }
}
