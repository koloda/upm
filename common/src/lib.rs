//! Ядро UPM — універсального менеджера всього, що встановлено з різних джерел.
//!
//! Його спільно використовують CLI та графічний інтерфейс:
//! - [`engine`] — операції list, update, upgrade;
//! - [`scope`] — вибір джерел і пакунків;
//! - [`report::Reporter`] — куди ядро повідомляє про хід роботи;
//! - [`sources`] — самі джерела пакунків.

pub mod catalog;
pub mod context;
pub mod engine;
pub mod report;
pub mod scope;
pub mod source;
pub mod sources;
pub mod version;

/// Чи запущено з правами root (наприклад, через sudo). Тоді brew, npm, go
/// тощо працювали б не для того користувача.
pub fn running_as_root() -> bool {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        // Поля: реальний, ефективний, збережений, файловий UID.
        .and_then(|ids| ids.split_whitespace().nth(1))
        == Some("0")
}
