//! Порівняння версій із різних світів: semver, Go pseudo-version, "21.0.8-tem".

use std::cmp::Ordering;

/// Чи новіша `latest` за `current`.
pub fn is_newer(current: &str, latest: &str) -> bool {
    compare(latest, current) == Ordering::Greater
}

pub fn compare(a: &str, b: &str) -> Ordering {
    let (release_a, pre_a) = split(a);
    let (release_b, pre_b) = split(b);
    natural(release_a, release_b).then_with(|| match (pre_a, pre_b) {
        (None, None) => Ordering::Equal,
        // 1.0.0 новіша за 1.0.0-beta.
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => natural(x, y),
    })
}

/// Ділить на основну частину та суфікс після першого дефіса; "+build" відкидається.
fn split(version: &str) -> (&str, Option<&str>) {
    let version = version.strip_prefix('v').unwrap_or(version);
    let version = version.split('+').next().unwrap_or(version);
    match version.split_once('-') {
        Some((release, pre)) => (release, Some(pre)),
        None => (version, None),
    }
}

/// Порівнює шматками: числа — як числа, літери — як текст.
fn natural(a: &str, b: &str) -> Ordering {
    let (a, b) = (chunks(a), chunks(b));
    for (x, y) in a.iter().zip(&b) {
        let numeric = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
        let order = match (numeric(x), numeric(y)) {
            (true, true) => {
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                x.len().cmp(&y.len()).then_with(|| x.cmp(y))
            }
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    a.len().cmp(&b.len())
}

fn chunks(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    let mut digits = false;
    for (i, c) in s.char_indices() {
        let alnum = c.is_alphanumeric();
        if let Some(from) = start
            && (!alnum || c.is_ascii_digit() != digits)
        {
            out.push(&s[from..i]);
            start = None;
        }
        if alnum && start.is_none() {
            start = Some(i);
            digits = c.is_ascii_digit();
        }
    }
    if let Some(from) = start {
        out.push(&s[from..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_compare_as_numbers() {
        assert!(is_newer("1.9.0", "1.10.0"));
        assert!(!is_newer("1.10.0", "1.9.0"));
        assert!(is_newer("0.84.3", "1.6.1"));
    }

    #[test]
    fn equal_versions_are_not_newer() {
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("v1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.3+build1", "1.2.3+build2"));
    }

    #[test]
    fn longer_release_is_newer() {
        assert!(is_newer("1.2", "1.2.1"));
    }

    #[test]
    fn final_release_beats_prerelease() {
        assert!(is_newer("1.0.0-beta.2", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0-beta.2"));
        assert!(is_newer("1.0.0-beta.2", "1.0.0-beta.10"));
    }

    #[test]
    fn go_pseudo_versions_order_by_date() {
        assert!(is_newer(
            "v0.0.0-20260622190343-b1d6741291d4",
            "v0.0.0-20261118162050-0eef789c9f9a"
        ));
    }
}
