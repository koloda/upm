//! `upm` — командний рядок над ядром upm-common.

mod args;
mod output;
mod render;

use std::path::{Path, PathBuf};

use upm_common::context::Context;
use upm_common::{engine, sources};

use args::Action;
use output::Terminal;

/// Виконує команду й повертає код завершення.
fn run(program: &str, args: &[String]) -> i32 {
    let request = match args::parse(args) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("Довідка: {program} help");
            return 2;
        }
    };
    if request.action == Action::Help {
        output::line(&args::usage(program));
        return 0;
    }
    if upm_common::running_as_root() {
        eprintln!(
            "Не запускайте через sudo: інакше brew, npm, go тощо працюватимуть не для вашого користувача."
        );
        return 1;
    }
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("Не задано змінну HOME");
        return 1;
    };

    let mut ctx = Context::new(home, request.dry, request.yes, Box::new(Terminal));
    // Якщо обробник не зареєструється, Ctrl+C просто завершить програму.
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(signal, ctx.cancel_flag());
    }
    let sources = sources::all();
    let scope = &request.scope;

    if request.action == Action::List {
        return render::list(&mut ctx, &request, &sources);
    }
    let picks = if scope.targets.is_empty() {
        Default::default()
    } else {
        match engine::resolve(&mut ctx, scope, &sources) {
            Ok(picks) => picks,
            Err(missing) => {
                eprintln!(
                    "Не знайдено: {}. Пошук: {program} list <частина назви>",
                    missing.join(" ")
                );
                eprintln!("Нічого не змінено.");
                return 1;
            }
        }
    };
    match request.action {
        Action::Update => render::update(&mut ctx, scope, &picks, &sources, program),
        _ => render::upgrade(&mut ctx, scope, &picks, &sources, program),
    }
}

fn main() {
    let mut args = std::env::args();
    // Довідка показує ту назву, під якою програму запустили.
    let program = args
        .next()
        .and_then(|path| Some(Path::new(&path).file_name()?.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "upm".to_string());
    let args: Vec<String> = args.collect();
    std::process::exit(run(&program, &args));
}
