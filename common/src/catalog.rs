//! Зведений каталог: установлене разом із результатами перевірки оновлень.
//! Це «модель екрана» для будь-якого інтерфейсу: фільтри, лічильники та план
//! оновлення. Тут немає ні Qt, ні термінала.

use std::collections::BTreeMap;

use crate::engine::SourceCheck;
use crate::scope::{Picks, Scope};
use crate::source::{Item, Update};
use crate::sources;

/// Стан пакунка щодо оновлень.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    NotChecked,
    Upgradable,
    UpToDate,
    /// Джерело не вміє перевіряти, або перевірка не вдалася.
    Unknown,
}

/// Рядок таблиці пакунків.
#[derive(Clone, Debug)]
pub struct Row {
    pub source: &'static str,
    pub key: String,
    pub name: String,
    pub current: String,
    /// Нова версія; порожньо, якщо оновлення не знайдено.
    pub latest: String,
    pub info: String,
    pub state: State,
}

pub struct Filter<'a> {
    /// `None` — усі джерела.
    pub source: Option<&'a str>,
    pub only_upgradable: bool,
    pub search: &'a str,
}

/// Підсумок по джерелу для бічної панелі.
#[derive(Clone, Copy, Debug)]
pub struct SourceSummary {
    pub id: &'static str,
    pub installed: usize,
    pub updates: usize,
    pub checked: bool,
    pub failed: bool,
}

/// Що буде зроблено кнопкою «Upgrade».
pub struct Plan {
    pub scope: Scope,
    pub picks: Picks,
    /// Пакунки для вікна підтвердження.
    pub preview: Vec<Row>,
    runnable: bool,
}

impl Plan {
    /// Нічого запускати.
    pub fn is_empty(&self) -> bool {
        !self.runnable
    }
}

struct Entry {
    id: &'static str,
    items: Vec<Item>,
    check: Option<SourceCheck>,
}

impl Entry {
    fn rows(&self) -> Vec<Row> {
        let updates: &[Update] = self.check.as_ref().map_or(&[], |check| &check.updates);
        let mut used = vec![false; updates.len()];
        let mut rows: Vec<Row> = self
            .items
            .iter()
            .map(|item| {
                let unique = self.items.iter().filter(|i| i.key == item.key).count() == 1;
                // Однаковий ключ мають, наприклад, пакунки npm у різних префіксах —
                // тоді їх розрізняє опис.
                let update = updates.iter().position(|update| {
                    update.key == item.key
                        && (unique || update.info.is_empty() || update.info == item.info)
                });
                if let Some(index) = update {
                    used[index] = true;
                }
                Row {
                    source: self.id,
                    key: item.key.clone(),
                    name: item.name.clone(),
                    current: item.version.clone(),
                    latest: update.map(|i| updates[i].latest.clone()).unwrap_or_default(),
                    info: item.info.clone(),
                    state: self.state(&item.key, update.is_some()),
                }
            })
            .collect();
        // Оновлення залежностей, яких немає серед установленого вами.
        rows.extend(
            updates
                .iter()
                .zip(used)
                .filter(|(_, used)| !used)
                .map(|(update, _)| Row {
                    source: self.id,
                    key: update.key.clone(),
                    name: update.name.clone(),
                    current: update.current.clone(),
                    latest: update.latest.clone(),
                    info: update.info.clone(),
                    state: State::Upgradable,
                }),
        );
        rows.sort_by_key(|row| row.name.to_lowercase());
        rows
    }

    fn state(&self, key: &str, has_update: bool) -> State {
        if self.id == "manual" {
            return State::Unknown;
        }
        match &self.check {
            None => State::NotChecked,
            Some(_) if has_update => State::Upgradable,
            Some(check)
                if check.failed
                    || !check.available
                    || check.unchecked.iter().any(|k| k == key) =>
            {
                State::Unknown
            }
            Some(_) => State::UpToDate,
        }
    }
}

#[derive(Default)]
pub struct Catalog {
    /// Ключ — позиція джерела в загальному порядку.
    entries: BTreeMap<usize, Entry>,
}

impl Catalog {
    fn entry(&mut self, id: &'static str) -> &mut Entry {
        let order = sources::NAMES
            .iter()
            .position(|name| *name == id)
            .unwrap_or(sources::NAMES.len());
        self.entries.entry(order).or_insert_with(|| Entry {
            id,
            items: Vec::new(),
            check: None,
        })
    }

    /// Новий список установленого; попередня перевірка вже не чинна.
    pub fn set_items(&mut self, source: &'static str, items: Vec<Item>) {
        let entry = self.entry(source);
        entry.items = items;
        entry.check = None;
    }

    pub fn set_check(&mut self, check: SourceCheck) {
        let entry = self.entry(check.source);
        entry.check = Some(check);
    }

    pub fn rows(&self, filter: &Filter) -> Vec<Row> {
        let search = filter.search.trim().to_lowercase();
        self.entries
            .values()
            .filter(|entry| filter.source.is_none_or(|source| source == entry.id))
            .flat_map(Entry::rows)
            .filter(|row| !filter.only_upgradable || row.state == State::Upgradable)
            .filter(|row| row.name.to_lowercase().contains(&search))
            .collect()
    }

    pub fn summaries(&self) -> Vec<SourceSummary> {
        self.entries
            .values()
            .map(|entry| SourceSummary {
                id: entry.id,
                installed: entry.items.len(),
                updates: entry.check.as_ref().map_or(0, |check| check.updates.len()),
                checked: entry.check.is_some(),
                failed: entry.check.as_ref().is_some_and(|check| check.failed),
            })
            .collect()
    }

    /// Позначене (пари "джерело, ключ"), а якщо нічого не позначено — усе
    /// у вибраному джерелі.
    pub fn upgrade_plan(&self, current: Option<&str>, selected: &[(String, String)]) -> Plan {
        let everything = Filter {
            source: None,
            only_upgradable: false,
            search: "",
        };
        let mut scope = Scope::default();
        let mut picks = Picks::new();

        if !selected.is_empty() {
            let preview: Vec<Row> = self
                .rows(&everything)
                .into_iter()
                .filter(|row| row.source != "manual")
                .filter(|row| {
                    selected
                        .iter()
                        .any(|(source, key)| source == row.source && *key == row.key)
                })
                .collect();
            for row in &preview {
                let keys = picks.entry(row.source).or_default();
                if !keys.contains(&row.key) {
                    keys.push(row.key.clone());
                }
            }
            scope.firmware = picks.contains_key("firmware");
            // Порожній вибір ядро зрозуміло б як "усе", тому такий план не запускається.
            let runnable = !picks.is_empty();
            return Plan {
                scope,
                picks,
                preview,
                runnable,
            };
        }

        let current = current.and_then(sources::canonical);
        if let Some(source) = current {
            scope.sources.push(source);
            scope.firmware = source == "firmware";
        }
        let preview = self
            .rows(&Filter {
                source: current,
                only_upgradable: true,
                search: "",
            })
            .into_iter()
            .filter(|row| scope.in_scope(row.source))
            .collect();
        Plan {
            runnable: current != Some("manual"),
            scope,
            picks,
            preview,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::SourceCheck;
    use crate::source::{Item, Update};

    fn check(source: &'static str, updates: Vec<Update>, unchecked: &[&str]) -> SourceCheck {
        SourceCheck {
            source,
            available: true,
            updates,
            failed: false,
            unchecked: unchecked.iter().map(|key| key.to_string()).collect(),
            without_updates: Vec::new(),
            targeted: false,
        }
    }

    fn sample() -> Catalog {
        let mut catalog = Catalog::default();
        // Навмисно не в тому порядку, в якому джерела показуються.
        catalog.set_items(
            "tools",
            vec![Item::new("claude", "2.0"), Item::new("droid", "0.1")],
        );
        catalog.set_items(
            "npm",
            vec![
                Item::new("cline", "3.0").info("/usr/local"),
                Item::new("codex", "1.0").info("/usr/local"),
                Item::new("codex", "2.0").info("/home/u"),
            ],
        );
        catalog.set_items("dnf", vec![Item::new("bash", "5.3")]);
        catalog.set_items("manual", vec![Item::new("zed", "")]);
        catalog.set_items("firmware", vec![Item::new("SSD", "1.0").key("abc")]);
        catalog
    }

    fn checked() -> Catalog {
        let mut catalog = sample();
        catalog.set_check(check(
            "npm",
            vec![Update::new("codex", "1.0", "2.0").info("/usr/local")],
            &[],
        ));
        // Залежність, якої немає у списку встановленого вами.
        catalog.set_check(check("dnf", vec![Update::new("libdep", "1", "2")], &[]));
        catalog.set_check(check(
            "tools",
            vec![Update::new("droid", "0.1", "0.2")],
            &["claude"],
        ));
        catalog.set_check(check(
            "firmware",
            vec![Update::new("SSD", "1.0", "1.1").key("abc")],
            &[],
        ));
        catalog
    }

    fn rows(catalog: &Catalog, source: Option<&str>, only: bool, search: &str) -> Vec<Row> {
        catalog.rows(&Filter {
            source,
            only_upgradable: only,
            search,
        })
    }

    fn names(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|row| format!("{}:{}", row.source, row.name))
            .collect()
    }

    fn selected(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(source, key)| (source.to_string(), key.to_string()))
            .collect()
    }

    #[test]
    fn all_view_follows_source_order_and_hides_nothing() {
        assert_eq!(
            names(&rows(&sample(), None, false, "")),
            [
                "dnf:bash",
                "npm:cline",
                "npm:codex",
                "npm:codex",
                "tools:claude",
                "tools:droid",
                "firmware:SSD",
                "manual:zed"
            ]
        );
    }

    #[test]
    fn source_and_search_filters_combine() {
        let catalog = sample();
        assert_eq!(rows(&catalog, Some("npm"), false, "").len(), 3);
        assert_eq!(names(&rows(&catalog, None, false, "CLI")), ["npm:cline"]);
        assert!(rows(&catalog, Some("dnf"), false, "cline").is_empty());
    }

    #[test]
    fn nothing_is_upgradable_before_check() {
        let catalog = sample();
        assert!(rows(&catalog, None, true, "").is_empty());
        assert!(
            rows(&catalog, Some("npm"), false, "")
                .iter()
                .all(|row| row.state == State::NotChecked)
        );
    }

    #[test]
    fn check_marks_states() {
        let catalog = checked();
        let state = |source: &str, name: &str, version: &str| {
            rows(&catalog, Some(source), false, "")
                .into_iter()
                .find(|row| row.name == name && row.current == version)
                .map(|row| (row.state, row.latest))
                .unwrap()
        };
        assert_eq!(state("npm", "cline", "3.0"), (State::UpToDate, "".into()));
        // Той самий пакунок у двох префіксах: оновлення лише в одному.
        assert_eq!(state("npm", "codex", "1.0"), (State::Upgradable, "2.0".into()));
        assert_eq!(state("npm", "codex", "2.0").0, State::UpToDate);
        assert_eq!(state("tools", "claude", "2.0").0, State::Unknown);
        assert_eq!(state("manual", "zed", "-").0, State::Unknown);
    }

    #[test]
    fn updates_of_unlisted_dependencies_are_shown() {
        assert_eq!(
            names(&rows(&checked(), None, true, "")),
            ["dnf:libdep", "npm:codex", "tools:droid", "firmware:SSD"]
        );
    }

    #[test]
    fn failed_check_leaves_state_unknown() {
        let mut catalog = sample();
        let mut result = check("npm", Vec::new(), &[]);
        result.failed = true;
        catalog.set_check(result);
        assert!(
            rows(&catalog, Some("npm"), false, "")
                .iter()
                .all(|row| row.state == State::Unknown)
        );
    }

    #[test]
    fn reloading_a_source_forgets_its_check() {
        let mut catalog = checked();
        catalog.set_items("npm", vec![Item::new("codex", "2.0")]);
        assert_eq!(
            rows(&catalog, Some("npm"), false, "")[0].state,
            State::NotChecked
        );
    }

    #[test]
    fn summaries_count_installed_and_updates() {
        let summaries = checked().summaries();
        let ids: Vec<_> = summaries.iter().map(|s| s.id).collect();
        assert_eq!(ids, ["dnf", "npm", "tools", "firmware", "manual"]);
        assert_eq!((summaries[0].installed, summaries[0].updates), (1, 1));
        assert_eq!((summaries[1].installed, summaries[1].updates), (3, 1));
    }

    #[test]
    fn selection_upgrades_only_selected() {
        let plan = checked().upgrade_plan(None, &selected(&[("npm", "codex")]));
        assert_eq!(plan.picks.len(), 1);
        assert_eq!(plan.picks["npm"], ["codex"]);
        assert!(plan.scope.sources.is_empty());
        assert_eq!(plan.preview.len(), 2);
        assert!(!plan.is_empty());
    }

    #[test]
    fn selected_firmware_is_allowed() {
        let plan = checked().upgrade_plan(None, &selected(&[("firmware", "abc")]));
        assert!(plan.scope.in_scope("firmware"));
    }

    #[test]
    fn nothing_selected_upgrades_current_source() {
        let plan = checked().upgrade_plan(Some("npm"), &[]);
        assert_eq!(plan.scope.sources, ["npm"]);
        assert!(plan.picks.is_empty());
        assert_eq!(names(&plan.preview), ["npm:codex"]);
    }

    #[test]
    fn nothing_selected_in_all_upgrades_everything_but_firmware() {
        let plan = checked().upgrade_plan(None, &[]);
        assert!(plan.scope.is_everything() && plan.picks.is_empty());
        assert_eq!(
            names(&plan.preview),
            ["dnf:libdep", "npm:codex", "tools:droid"]
        );
    }

    #[test]
    fn manual_source_cannot_be_upgraded() {
        assert!(checked().upgrade_plan(Some("manual"), &[]).is_empty());
        assert!(
            checked()
                .upgrade_plan(None, &selected(&[("manual", "zed")]))
                .is_empty()
        );
    }

    #[test]
    fn unknown_selection_never_becomes_full_upgrade() {
        // Застарілий вибір (пакунка вже немає) не повинен означати "оновити все".
        let plan = checked().upgrade_plan(None, &selected(&[("npm", "gone")]));
        assert!(plan.is_empty());
    }
}
