//! Реєстр джерел. Порядок тут — порядок виконання й виводу.

mod apps;
mod lang;
mod system;

use crate::source::Source;

pub fn all() -> Vec<Box<dyn Source>> {
    vec![
        Box::new(system::Dnf),
        Box::new(system::Flatpak),
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
pub fn removable(id: &str) -> bool {
    all().iter().any(|source| source.id() == id && source.can_remove())
}

pub const NAMES: [&str; 15] = [
    "dnf", "flatpak", "brew", "appimage", "npm", "composer", "go", "rust", "python", "tools",
    "sdkman", "android", "shell", "firmware", "manual",
];

/// Назва джерела або синонім -> канонічна назва.
pub fn canonical(word: &str) -> Option<&'static str> {
    let word = word.to_lowercase();
    let word = match word.as_str() {
        "node" | "nodejs" => "npm",
        "py" | "pip" => "python",
        "cargo" => "rust",
        "rpm" => "dnf",
        "golang" => "go",
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

    #[test]
    fn names_match_registry() {
        let ids: Vec<_> = all().iter().map(|source| source.id()).collect();
        assert_eq!(ids, NAMES);
    }

    #[test]
    fn aliases_resolve_to_sources() {
        assert_eq!(canonical("Node"), Some("npm"));
        assert_eq!(canonical("cargo"), Some("rust"));
        assert_eq!(canonical("firefox"), None);
    }
}
