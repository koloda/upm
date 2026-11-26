//! Реєстр джерел. Порядок тут — порядок виконання й виводу.

mod alpine;
mod apps;
mod arch;
mod deb;
mod lang;
mod rpm;
mod suse;
mod system;
mod universal;

use crate::context::{Cmd, Context};
use crate::platform::Platform;
use crate::scope::Scope;
use crate::source::{Source, Status};

/// Запит, вивід якого розбираємо: мова завжди англійська.
fn query<const N: usize>(argv: [&str; N]) -> Cmd {
    Cmd::new(argv).env("LC_ALL", "C")
}

/// Команда, що змінює пакунки. `yes` — прапорець «не перепитувати»,
/// який додається лише з `-y`.
fn change_cmd(ctx: &Context, argv: &[&str], yes: &str, packages: &[String]) -> Cmd {
    let mut cmd = Cmd::new(argv.iter().copied());
    if ctx.yes {
        cmd = cmd.args([yes]);
    }
    cmd.args(packages.to_vec())
}

/// Виконує дібрані команди видалення; після скасування нові не запускає.
fn run_removals(ctx: &mut Context, commands: Vec<Cmd>) -> Status {
    let total = commands.len();
    let mut success = true;
    let mut done = 0;
    for cmd in &commands {
        if ctx.interrupted() {
            break;
        }
        done += 1;
        success &= ctx.run(cmd);
    }
    if done == 0 {
        Status::Skipped
    } else if done < total {
        Status::Failed
    } else {
        Status::from_success(success)
    }
}

/// Без термінала й без `-y` підтверджувати нікому, а pacman та apk на кінці
/// вводу не відмовляються як слід: pacman «успішно» нічого не робить, apk —
/// виконує. Тому такі зміни не запускаємо.
fn confirmable(ctx: &mut Context, id: &str) -> bool {
    let possible = ctx.yes || ctx.dry || ctx.interactive();
    if !possible {
        ctx.note(format!(
            "{id}: немає термінала для підтвердження (і не задано -y) — пропущено"
        ));
    }
    possible
}

/// Усі джерела; `platform` визначає, які з них лише читають.
pub fn all(platform: &Platform) -> Vec<Box<dyn Source>> {
    vec![
        Box::new(rpm::Dnf),
        Box::new(rpm::Yum),
        Box::new(suse::Zypper),
        Box::new(rpm::PlainRpm),
        Box::new(deb::Apt),
        Box::new(arch::Pacman {
            read_only: platform.immutable,
        }),
        Box::new(arch::Aur),
        Box::new(alpine::Apk),
        Box::new(system::Flatpak),
        Box::new(universal::Snap),
        Box::new(universal::Nix),
        Box::new(system::Brew),
        Box::new(system::AppImage),
        Box::new(lang::Npm),
        Box::new(lang::Composer),
        Box::new(lang::Go),
        Box::new(lang::Rust),
        Box::new(lang::Python),
        Box::new(apps::Tools),
        Box::new(apps::Sdkman),
        Box::new(apps::Android),
        Box::new(apps::Shell),
        Box::new(system::Firmware),
        Box::new(apps::Manual),
    ]
}

/// Чи може джерело видаляти свої пакунки (без запуску будь-якої команди).
/// Використовується планом видалення, щоб не пропонувати неможливе.
pub fn removable(id: &str, platform: &Platform) -> bool {
    all(platform)
        .iter()
        .any(|source| source.id() == id && source.can_remove())
}

pub const NAMES: [&str; 24] = [
    "dnf", "yum", "zypper", "rpm", "apt", "pacman", "aur", "apk", "flatpak", "snap", "nix",
    "brew", "appimage", "npm", "composer", "go", "rust", "python", "tools", "sdkman", "android",
    "shell", "firmware", "manual",
];

/// Слово "rpm" означає те RPM-джерело, що діє на цьому комп'ютері; якщо
/// жодного, лишається загальне "rpm", і ядро скаже, що його тут немає.
pub fn for_host(ctx: &Context, id: &'static str) -> &'static str {
    if id != "rpm" {
        return id;
    }
    let candidates: [Box<dyn Source>; 3] = [
        Box::new(rpm::Dnf),
        Box::new(rpm::Yum),
        Box::new(suse::Zypper),
    ];
    candidates
        .iter()
        .find(|source| source.available(ctx))
        .map_or("rpm", |source| source.id())
}

/// Переводить назви джерел у виборі на джерела цього комп'ютера.
pub fn map_to_host(ctx: &Context, scope: &mut Scope) {
    let lists = [&mut scope.only, &mut scope.skip, &mut scope.sources];
    for id in lists.into_iter().flatten() {
        *id = for_host(ctx, id);
    }
    for target in &mut scope.targets {
        target.source = target.source.map(|id| for_host(ctx, id));
    }
}

/// Назва джерела або синонім -> канонічна назва.
pub fn canonical(word: &str) -> Option<&'static str> {
    let word = word.to_lowercase();
    let word = match word.as_str() {
        "node" | "nodejs" => "npm",
        "py" | "pip" => "python",
        "cargo" => "rust",
        "golang" => "go",
        "deb" | "dpkg" => "apt",
        "homebrew" => "brew",
        "appimages" => "appimage",
        "sdk" => "sdkman",
        other => other,
    };
    NAMES.into_iter().find(|name| *name == word)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::Target;

    #[test]
    fn names_match_registry() {
        let ids: Vec<_> = all(&Platform::default()).iter().map(|source| source.id()).collect();
        assert_eq!(ids, NAMES);
    }

    #[test]
    fn rpm_means_the_hosts_rpm_source() {
        use crate::testing::Host;
        for (release, tools, expected) in [
            ("ID=fedora\n", &["rpm", "dnf"][..], "dnf"),
            ("ID=centos\n", &["rpm", "yum"][..], "yum"),
            ("ID=opensuse-leap\n", &["rpm", "zypper"][..], "zypper"),
            ("ID=fedora\n", &["rpm"][..], "rpm"),
            ("ID=debian\n", &["apt-get"][..], "rpm"),
        ] {
            let host = Host::with(release, tools);
            assert_eq!(for_host(&host.ctx, "rpm"), expected, "{release}");
            // Інші назви не чіпаються.
            assert_eq!(for_host(&host.ctx, "npm"), "npm");
        }
    }

    #[test]
    fn host_mapping_rewrites_every_part_of_a_scope() {
        use crate::testing::Host;
        let host = Host::with("ID=fedora\n", &["rpm", "dnf"]);
        let mut scope = Scope {
            only: vec!["rpm", "npm"],
            skip: vec!["rpm"],
            sources: vec!["rpm"],
            targets: vec![
                Target {
                    source: Some("rpm"),
                    query: "bash".into(),
                },
                Target {
                    source: None,
                    query: "rpm".into(),
                },
            ],
            ..Scope::default()
        };
        map_to_host(&host.ctx, &mut scope);
        assert_eq!(scope.only, ["dnf", "npm"]);
        assert_eq!(scope.skip, ["dnf"]);
        assert_eq!(scope.sources, ["dnf"]);
        assert_eq!(scope.targets[0].source, Some("dnf"));
        assert_eq!(scope.targets[1].query, "rpm");
    }

    #[test]
    fn every_source_name_and_synonym_resolves() {
        for name in NAMES {
            assert_eq!(canonical(name), Some(name));
        }
        for (word, id) in [
            ("node", "npm"),
            ("nodejs", "npm"),
            ("py", "python"),
            ("pip", "python"),
            ("cargo", "rust"),
            ("golang", "go"),
            ("homebrew", "brew"),
            ("appimages", "appimage"),
            ("sdk", "sdkman"),
            ("DEB", "apt"),
            ("dpkg", "apt"),
        ] {
            assert_eq!(canonical(word), Some(id), "{word}");
        }
    }

    #[test]
    fn removal_support_follows_the_source_and_the_host() {
        let ordinary = Platform::default();
        let kept: Vec<&str> = NAMES
            .into_iter()
            .filter(|id| !removable(id, &ordinary))
            .collect();
        assert_eq!(kept, ["rpm", "go", "tools", "firmware", "manual"]);
        let mut immutable = ordinary.clone();
        immutable.immutable = true;
        assert!(!removable("pacman", &immutable));
    }

    #[test]
    fn aliases_resolve_to_sources() {
        assert_eq!(canonical("rpm"), Some("rpm"));
        assert_eq!(canonical("RPM"), Some("rpm"));
        assert_eq!(canonical("Node"), Some("npm"));
        assert_eq!(canonical("cargo"), Some("rust"));
        assert_eq!(canonical("deb"), Some("apt"));
        assert_eq!(canonical("dpkg"), Some("apt"));
        assert_eq!(canonical("firefox"), None);
    }
}
