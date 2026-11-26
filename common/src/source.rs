//! Спільний інтерфейс усіх джерел пакунків.

use crate::context::Context;

/// Установлений пакунок.
#[derive(Clone, Debug)]
pub struct Item {
    pub name: String,
    pub version: String,
    /// Опис: шлях, репозиторій, вид пакунка.
    pub info: String,
    /// За цим ключем джерело оновлює пакунок.
    pub key: String,
    /// Скорочені назви, за якими пакунок теж можна знайти.
    pub aliases: Vec<String>,
}

impl Item {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        let name = name.into();
        let version = version.into();
        Item {
            key: name.clone(),
            name,
            version: if version.is_empty() { "-".into() } else { version },
            info: String::new(),
            aliases: Vec::new(),
        }
    }

    pub fn info(mut self, info: impl Into<String>) -> Self {
        self.info = info.into();
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }
}

/// Доступне оновлення.
#[derive(Clone, Debug)]
pub struct Update {
    pub name: String,
    pub current: String,
    pub latest: String,
    pub key: String,
    pub info: String,
}

impl Update {
    pub fn new(name: &str, current: &str, latest: &str) -> Self {
        Update {
            name: name.to_string(),
            current: if current.is_empty() { "-".into() } else { current.into() },
            latest: latest.to_string(),
            key: name.to_string(),
            info: String::new(),
        }
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = key.into();
        self
    }

    pub fn info(mut self, info: impl Into<String>) -> Self {
        self.info = info.into();
        self
    }
}

/// Результат запиту. `failed` — щось не вдалося, але зібрані рядки дійсні.
pub struct Query<T> {
    pub rows: Vec<T>,
    pub failed: bool,
    /// Ключі пакунків, стан яких з'ясувати не вдалося.
    pub unchecked: Vec<String>,
}

impl<T> Query<T> {
    pub fn new() -> Self {
        Query {
            rows: Vec::new(),
            failed: false,
            unchecked: Vec::new(),
        }
    }

    pub fn failure() -> Self {
        Query {
            rows: Vec::new(),
            failed: true,
            unchecked: Vec::new(),
        }
    }
}

impl<T> Default for Query<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> From<Vec<T>> for Query<T> {
    fn from(rows: Vec<T>) -> Self {
        Query {
            rows,
            failed: false,
            unchecked: Vec::new(),
        }
    }
}

/// Що вибрано в межах одного джерела: усе або конкретні ключі.
#[derive(Clone, Copy)]
pub struct Pick<'a>(pub Option<&'a [String]>);

impl<'a> Pick<'a> {
    pub fn targeted(&self) -> bool {
        self.0.is_some()
    }

    pub fn has(&self, key: &str) -> bool {
        self.0.is_none_or(|keys| keys.iter().any(|k| k == key))
    }

    /// Вибрані ключі; порожньо, якщо вибрано все джерело.
    pub fn keys(&self) -> &'a [String] {
        self.0.unwrap_or_default()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    Skipped,
    Failed,
}

impl Status {
    pub fn from_success(success: bool) -> Self {
        if success { Status::Ok } else { Status::Failed }
    }
}

/// Одне джерело пакунків. Щоб додати нове, досить реалізувати цей trait
/// і внести його до `sources::all()`.
pub trait Source {
    fn id(&self) -> &'static str;

    /// Чи є це джерело на комп'ютері.
    fn available(&self, ctx: &Context) -> bool;

    /// Установлене. `all` — разом із залежностями; `versions` — чи потрібні
    /// версії там, де їх дорого визначати.
    fn inventory(&self, ctx: &mut Context, all: bool, versions: bool) -> Query<Item>;

    /// Доступні оновлення. Нічого з установленого не змінює.
    fn outdated(&self, ctx: &mut Context, pick: Pick) -> Query<Update>;

    fn upgrade(&self, ctx: &mut Context, pick: Pick) -> Status;

    /// Чи вміє джерело видаляти свої пакунки власною командою.
    /// Джерела без такої команди не видаляють файли самі.
    fn can_remove(&self) -> bool {
        false
    }

    /// Видаляє вибране. Типова реалізація нічого не робить і повідомляє
    /// `Skipped`; джерела з власною командою перевизначають її.
    fn remove(&self, _ctx: &mut Context, _pick: Pick) -> Status {
        Status::Skipped
    }

    /// Чи варто заздалегідь запитати пароль sudo перед повним оновленням.
    fn needs_sudo(&self) -> bool {
        false
    }
}
