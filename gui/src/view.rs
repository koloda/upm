//! Перетворення даних ядра на те, що показує QML (JSON), і назад.

use serde_json::json;
use upm_common::catalog::{Catalog, DeletionPlan, Filter, Plan, State};

pub fn title(id: &str) -> &str {
    match id {
        "" => "All",
        "dnf" => "DNF",
        "yum" => "YUM",
        "zypper" => "Zypper",
        "rpm" => "RPM",
        "apt" => "APT",
        "pacman" => "Pacman",
        "aur" => "AUR",
        "apk" => "APK",
        "flatpak" => "Flatpak",
        "snap" => "Snap",
        "nix" => "Nix",
        "brew" => "Homebrew",
        "appimage" => "AppImage",
        "npm" => "Node",
        "composer" => "Composer",
        "go" => "Go",
        "rust" => "Rust",
        "python" => "Python",
        "tools" => "Програми",
        "sdkman" => "SDKMAN",
        "android" => "Android",
        "shell" => "Плагіни",
        "firmware" => "Прошивки",
        "manual" => "Вручну",
        other => other,
    }
}

/// Бічна панель: "All", а далі кожне джерело.
pub fn sources(catalog: &Catalog) -> String {
    let summaries = catalog.summaries();
    let all = json!({
        "id": "",
        "title": title(""),
        "installed": summaries.iter().map(|s| s.installed).sum::<usize>(),
        "updates": summaries.iter().map(|s| s.updates).sum::<usize>(),
        "failed": summaries.iter().any(|s| s.failed),
    });
    let each = summaries.iter().map(|summary| {
        json!({
            "id": summary.id,
            "title": title(summary.id),
            "installed": summary.installed,
            "updates": summary.updates,
            "failed": summary.failed,
        })
    });
    serde_json::Value::Array(std::iter::once(all).chain(each).collect()).to_string()
}

pub fn rows(catalog: &Catalog, filter: &Filter) -> String {
    let rows = catalog.rows(filter).into_iter().map(|row| {
        let state = match row.state {
            State::NotChecked => "unchecked",
            State::Upgradable => "upgradable",
            State::UpToDate => "current",
            State::Unknown => "unknown",
        };
        json!({
            "source": row.source,
            "sourceTitle": title(row.source),
            "key": row.key,
            "name": row.name,
            "current": row.current,
            "latest": row.latest,
            "state": state,
            "info": row.info,
            "removable": row.removable,
        })
    });
    serde_json::Value::Array(rows.collect()).to_string()
}

/// Вікно підтвердження перед оновленням.
pub fn confirmation(plan: &Plan) -> String {
    let mut lines = Vec::new();
    let title = if !plan.picks.is_empty() {
        "Оновити вибрані пакунки?".to_string()
    } else if let [source] = plan.scope.sources[..] {
        lines.push(
            "Оновиться все в джерелі, навіть те, чого немає в переліку нижче.".to_string(),
        );
        format!("Оновити все в «{}»?", title(source))
    } else {
        lines.push(
            "Оновляться всі джерела (крім прошивок), навіть те, чого немає в переліку нижче."
                .to_string(),
        );
        "Оновити все?".to_string()
    };
    if plan.is_empty() {
        lines = vec!["Нічого оновлювати: це оновлюється вручну або вже зникло.".to_string()];
    } else {
        lines.extend(plan.preview.iter().map(|row| {
            let mut line = format!("{}: {} {}", self::title(row.source), row.name, row.current);
            if !row.latest.is_empty() {
                line.push_str(&format!(" → {}", row.latest));
            }
            line
        }));
    }
    json!({ "runnable": !plan.is_empty(), "title": title, "lines": lines }).to_string()
}

/// Вікно підтвердження перед видаленням. На відміну від оновлення, тут
/// немає режиму «все» — лише названі пакунки.
pub fn delete_confirmation(plan: &DeletionPlan) -> String {
    let (title, lines) = if plan.is_empty() {
        (
            "Нічого видаляти".to_string(),
            vec!["Нічого видаляти: це оновлюється вручну або вже зникло.".to_string()],
        )
    } else {
        (
            "Видалити вибрані пакунки?".to_string(),
            plan.preview
                .iter()
                .map(|row| format!("{}: {} {}", self::title(row.source), row.name, row.current))
                .collect(),
        )
    };
    json!({ "runnable": !plan.is_empty(), "title": title, "lines": lines }).to_string()
}

/// Пари "джерело, ключ" із QML. `None`, якщо дані пошкоджені: порожній
/// список означав би "оновити все", тому плутати їх не можна.
pub fn selection(json: &str) -> Option<Vec<(String, String)>> {
    serde_json::from_str(json).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use upm_common::source::Item;

    use super::*;

    fn catalog() -> Catalog {
        let mut catalog = Catalog::default();
        catalog.set_items("npm", vec![Item::new("codex", "1.0").info("/usr/local")]);
        catalog.set_items("manual", vec![Item::new("zed", "")]);
        catalog
    }

    fn parse(json: String) -> Value {
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn every_source_has_a_readable_title() {
        for (id, expected) in [
            ("yum", "YUM"),
            ("zypper", "Zypper"),
            ("rpm", "RPM"),
            ("apt", "APT"),
            ("pacman", "Pacman"),
            ("aur", "AUR"),
            ("apk", "APK"),
            ("snap", "Snap"),
            ("nix", "Nix"),
        ] {
            assert_eq!(title(id), expected);
        }
        for id in upm_common::sources::NAMES {
            assert_ne!(title(id), id, "без назви: {id}");
        }
    }

    #[test]
    fn sidebar_starts_with_all_and_totals() {
        let sidebar = parse(sources(&catalog()));
        assert_eq!(sidebar[0]["title"], "All");
        assert_eq!(sidebar[0]["installed"], 2);
        assert_eq!(sidebar[1]["title"], "Node");
    }

    #[test]
    fn rows_carry_state_and_titles() {
        let filter = Filter {
            source: Some("npm"),
            only_upgradable: false,
            search: "",
        };
        let parsed = parse(rows(&catalog(), &filter));
        assert_eq!(parsed[0]["sourceTitle"], "Node");
        assert_eq!(parsed[0]["state"], "unchecked");
        assert_eq!(parsed[0]["removable"], true);

        let manual = parse(rows(
            &catalog(),
            &Filter {
                source: Some("manual"),
                only_upgradable: false,
                search: "",
            },
        ));
        assert_eq!(manual[0]["removable"], false);
    }

    #[test]
    fn confirmation_names_what_will_happen() {
        let catalog = catalog();
        let picked = [("npm".to_string(), "codex".to_string())];
        let chosen = parse(confirmation(&catalog.upgrade_plan(None, &picked)));
        assert_eq!(chosen["title"], "Оновити вибрані пакунки?");
        assert_eq!(chosen["lines"][0], "Node: codex 1.0");

        let source = parse(confirmation(&catalog.upgrade_plan(Some("npm"), &[])));
        assert_eq!(source["title"], "Оновити все в «Node»?");

        let all = parse(confirmation(&catalog.upgrade_plan(None, &[])));
        assert_eq!(all["title"], "Оновити все?");
        assert_eq!(all["runnable"], true);

        let manual = parse(confirmation(&catalog.upgrade_plan(Some("manual"), &[])));
        assert_eq!(manual["runnable"], false);
    }

    #[test]
    fn deletion_confirmation_names_items_and_refuses_empty() {
        let catalog = catalog();
        let picked = [("npm".to_string(), "codex".to_string())];
        let chosen = parse(delete_confirmation(&catalog.deletion_plan(&picked)));
        assert_eq!(chosen["title"], "Видалити вибрані пакунки?");
        assert_eq!(chosen["lines"][0], "Node: codex 1.0");
        assert_eq!(chosen["runnable"], true);

        let manual = [("manual".to_string(), "zed".to_string())];
        let refused = parse(delete_confirmation(&catalog.deletion_plan(&manual)));
        assert_eq!(refused["runnable"], false);

        let empty = parse(delete_confirmation(&catalog.deletion_plan(&[])));
        assert_eq!(empty["runnable"], false);
    }

    #[test]
    fn broken_selection_is_not_an_empty_selection() {
        assert_eq!(selection("[]"), Some(Vec::new()));
        assert_eq!(selection("not json"), None);
        assert_eq!(selection(r#"[["npm"]]"#), None);
    }
}
