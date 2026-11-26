//! Показ результатів ядра в терміналі.

use upm_common::context::Context;
use upm_common::engine::{self, CheckEvent, SourceCheck, Summary};
use upm_common::scope::{Picks, Scope};
use upm_common::source::Source;

use crate::args::Request;
use crate::output::{self, Color, paint};

const INTERRUPTED: i32 = 130;

pub fn list(ctx: &mut Context, request: &Request, sources: &[Box<dyn Source>]) -> i32 {
    let mut total = 0;
    let outcome = engine::list(ctx, &request.scope, request.all, sources, |_, listing| {
        if listing.failed {
            output::warn(&format!("{}: не вдалося отримати список", listing.source));
        }
        if listing.items.is_empty() {
            if request.scope.named(listing.source) {
                output::header(listing.source);
                output::info("(нічого не встановлено)");
            }
            return;
        }
        output::header(&format!("{} ({})", listing.source, listing.items.len()));
        total += listing.items.len();
        let table: Vec<Vec<String>> = listing
            .items
            .into_iter()
            .map(|item| vec![item.name, item.version, item.info])
            .collect();
        output::table(&table);
    });
    if outcome.interrupted {
        return INTERRUPTED;
    }

    for query in &outcome.missing {
        eprintln!("Не знайдено: {query}");
    }
    let hint = if outcome.includes_dependencies {
        ""
    } else {
        " Показано встановлене вами; --all — разом із залежностями."
    };
    output::line("");
    output::line(&format!("Усього: {total}.{hint}"));
    i32::from(outcome.failed || !outcome.missing.is_empty())
}

fn show_check(check: &SourceCheck) {
    if !check.available {
        output::info("пропущено: тут цього джерела немає");
        return;
    }
    if check.failed {
        output::warn("перевірка завершилась з помилкою");
    }
    let table: Vec<Vec<String>> = check
        .updates
        .iter()
        .map(|u| {
            vec![
                u.name.clone(),
                u.current.clone(),
                u.latest.clone(),
                u.info.clone(),
            ]
        })
        .collect();
    output::table(&table);
    for name in &check.without_updates {
        output::info(&format!(
            "{name}: оновлень не знайдено (актуальний або джерело не вміє це перевірити)"
        ));
    }
    if !check.targeted && check.updates.is_empty() && !check.failed && check.source != "manual" {
        output::info("оновлень не знайдено");
    }
}

pub fn update(
    ctx: &mut Context,
    scope: &Scope,
    picks: &Picks,
    sources: &[Box<dyn Source>],
    program: &str,
) -> i32 {
    let mut total = 0;
    let mut failed = Vec::new();
    let finished = engine::check_updates(ctx, scope, picks, sources, |_, event| match event {
        CheckEvent::Started(id) => output::header(id),
        CheckEvent::Finished(check) => {
            total += check.updates.len();
            if check.failed {
                failed.push(check.source);
            }
            show_check(&check);
        }
    });
    if !finished {
        return INTERRUPTED;
    }

    output::line("");
    output::line(&format!(
        "{} Оновити: {program} upgrade [джерело чи пакунок]",
        paint(Color::Bold, &format!("Можна оновити: {total}."))
    ));
    if failed.is_empty() {
        0
    } else {
        output::info(&format!(
            "{} {}",
            paint(Color::Red, "Не вдалося перевірити:"),
            failed.join(" ")
        ));
        1
    }
}

/// Те, що не оновлюється автоматично.
fn show_manual_report(ctx: &mut Context, sources: &[Box<dyn Source>]) {
    output::header("Перевірте вручну (скрипт це не оновлює)");
    let report = engine::manual_report(ctx, sources);
    if !report.orphan_rpms.is_empty() {
        output::info("RPM-пакунки, яких немає в жодному увімкненому репозиторії:");
        let rows: Vec<Vec<String>> = report
            .orphan_rpms
            .into_iter()
            .map(|(name, version)| vec![String::new(), name, version])
            .collect();
        output::table(&rows);
    }
    let rows: Vec<Vec<String>> = report
        .applications
        .into_iter()
        .map(|item| vec![item.name, item.info])
        .collect();
    output::table(&rows);
    output::info(
        "AppImage без відомого джерела оновлень AppManager пропускає — вони оновлюються з самої програми",
    );
}

fn show_summary(ctx: &Context, summary: &Summary) {
    output::line("");
    output::line(&paint(
        Color::Bold,
        "==================== Підсумок ====================",
    ));
    if ctx.dry {
        output::info("Режим перегляду: нічого не змінено.");
    }
    for (color, title, list) in [
        (Color::Green, "Успішно:", &summary.ok),
        (Color::Yellow, "Пропущено:", &summary.skipped),
        (Color::Red, "З помилками:", &summary.failed),
    ] {
        if !list.is_empty() {
            output::info(&format!("{} {}", paint(color, title), list.join(" ")));
        }
    }
    if !summary.failed.is_empty() {
        for command in &ctx.failed_commands {
            output::info(&format!("  {} {command}", paint(Color::Red, "✗")));
        }
    }
    if !ctx.notes.is_empty() {
        output::info(&paint(Color::Yellow, "Зауваження:"));
        for note in &ctx.notes {
            output::info(&format!("  • {note}"));
        }
    }
}

pub fn upgrade(
    ctx: &mut Context,
    scope: &Scope,
    picks: &Picks,
    sources: &[Box<dyn Source>],
    program: &str,
) -> i32 {
    let summary = engine::upgrade(ctx, scope, picks, sources, |_, id| output::header(id));

    if summary.interrupted {
        output::line("");
        output::warn("Перервано");
        show_summary(ctx, &summary);
        return INTERRUPTED;
    }
    if scope.is_everything() {
        if !scope.firmware {
            output::header("firmware");
            output::info(&format!(
                "Пропущено. Щоб оновити прошивки: {program} upgrade --firmware"
            ));
        }
        show_manual_report(ctx, sources);
    }
    show_summary(ctx, &summary);
    i32::from(!summary.failed.is_empty())
}
