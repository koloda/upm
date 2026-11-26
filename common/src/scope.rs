//! Що саме вибрано для команди: джерела та пакунки.

use std::collections::HashMap;

use crate::source::Pick;

/// Названий пакунок, за потреби з указаним джерелом ("dnf:firefox").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub source: Option<&'static str>,
    pub query: String,
}

/// Порожній `Scope` означає «все, крім прошивок».
#[derive(Clone, Debug, Default)]
pub struct Scope {
    pub only: Vec<&'static str>,
    pub skip: Vec<&'static str>,
    /// Прошивки беруть участь лише на явний запит.
    pub firmware: bool,
    /// Вибрані джерела.
    pub sources: Vec<&'static str>,
    /// Вибрані пакунки.
    pub targets: Vec<Target>,
}

/// Вибрані ключі пакунків для кожного джерела.
pub type Picks = HashMap<&'static str, Vec<String>>;

pub fn pick<'a>(picks: &'a Picks, id: &str) -> Pick<'a> {
    Pick(picks.get(id).map(Vec::as_slice))
}

impl Scope {
    /// Чи дозволяють only/skip/firmware працювати з джерелом.
    pub fn in_scope(&self, id: &str) -> bool {
        (self.only.is_empty() || self.only.contains(&id))
            && !self.skip.contains(&id)
            && (id != "firmware" || self.firmware)
    }

    pub fn named(&self, id: &str) -> bool {
        self.sources.contains(&id)
    }

    /// Джерело не відсіяне переліком вибраних джерел.
    pub fn allows(&self, id: &str) -> bool {
        self.sources.is_empty() || self.named(id)
    }

    /// Чи є пакунки, названі без джерела.
    pub fn has_bare_target(&self) -> bool {
        self.targets.iter().any(|target| target.source.is_none())
    }

    /// Нічого конкретного не вибрано — це повний прохід.
    pub fn is_everything(&self) -> bool {
        self.sources.is_empty() && self.targets.is_empty()
    }

    /// Чи бере участь джерело в update/upgrade.
    pub fn is_active(&self, picks: &Picks, id: &str) -> bool {
        if !self.in_scope(id) {
            return false;
        }
        if self.targets.is_empty() && picks.is_empty() {
            self.allows(id)
        } else {
            // Вибрано конкретні пакунки — решта джерел не чіпається.
            picks.contains_key(id) || (!self.has_bare_target() && self.named(id))
        }
    }
}
