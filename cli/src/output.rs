//! Друк на екран: рядки, що не падають на закритому конвеєрі, кольори, таблиці.

use std::io::{IsTerminal, Write};
use std::sync::OnceLock;

use upm_common::report::Reporter;

/// Повідомлення ядра в терміналі.
pub struct Terminal;

impl Reporter for Terminal {
    fn info(&self, text: &str) {
        info(text);
    }

    fn warn(&self, text: &str) {
        warn(text);
    }

    fn diagnostic(&self, text: &str) {
        eprintln!("  ! {text}");
    }

    fn log(&self, text: &str) {
        eprint!("{text}");
    }

    fn command(&self, command: &str, side: bool) {
        if side {
            eprintln!("+ {command}");
        } else {
            line(&paint(Color::Blue, &format!("+ {command}")));
        }
    }

    fn failure(&self, problem: &str) {
        line(&paint(Color::Red, &format!("  {problem}")));
    }
}

/// Рядок у stdout. Якщо читач закрив конвеєр (`upm list | head`) — тихо виходимо.
pub fn line(text: &str) {
    if writeln!(std::io::stdout(), "{text}").is_err() {
        std::process::exit(0);
    }
}

#[derive(Clone, Copy)]
pub enum Color {
    Bold,
    Red,
    Green,
    Yellow,
    Blue,
}

pub fn paint(color: Color, text: &str) -> String {
    static TERMINAL: OnceLock<bool> = OnceLock::new();
    if !*TERMINAL.get_or_init(|| std::io::stdout().is_terminal()) {
        return text.to_string();
    }
    let code = match color {
        Color::Bold => 1,
        Color::Red => 31,
        Color::Green => 32,
        Color::Yellow => 33,
        Color::Blue => 34,
    };
    format!("\x1b[{code}m{text}\x1b[0m")
}

pub fn info(text: &str) {
    line(&format!("  {text}"));
}

pub fn warn(text: &str) {
    line(&format!("  {}", paint(Color::Yellow, &format!("! {text}"))));
}

pub fn header(text: &str) {
    line("");
    line(&paint(Color::Bold, &format!("==> {text}")));
}

/// Вирівнює колонки за кількістю символів.
pub fn table(rows: &[Vec<String>]) {
    for text in format_table(rows) {
        line(&text);
    }
}

fn format_table(rows: &[Vec<String>]) -> Vec<String> {
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        widths.resize(widths.len().max(row.len()), 0);
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            let mut text = String::new();
            for (cell, width) in row.iter().zip(&widths) {
                text.push_str(&format!("  {cell:<width$}"));
            }
            text.trim_end().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_align_with_cyrillic_text() {
        let rows = vec![
            vec!["Базар".to_string(), "1".to_string(), String::new()],
            vec!["ab".to_string(), "22".to_string(), "x".to_string()],
        ];
        assert_eq!(format_table(&rows), ["  Базар  1", "  ab     22  x"]);
    }
}
