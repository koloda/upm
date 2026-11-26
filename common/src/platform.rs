//! Що це за комп'ютер: сімейство дистрибутива та чи можна змінювати
//! системні пакунки. Визначається один раз; джерела лише питають.

use std::path::{Path, PathBuf};

/// Сімейство дистрибутивів зі спільним менеджером пакунків.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// Fedora, RHEL та похідні.
    Rpm,
    /// openSUSE та SLE.
    Suse,
    Debian,
    Arch,
    Alpine,
}

/// Частини ID незмінних варіантів openSUSE та SUSE.
const IMMUTABLE_SUSE: [&str; 6] = [
    "microos",
    "leap-micro",
    "sl-micro",
    "sle-micro",
    "aeon",
    "kalpa",
];

fn family_of(id: &str) -> Option<Family> {
    Some(match id {
        "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "amzn" | "ol" | "mageia"
        | "altlinux" => Family::Rpm,
        "suse" | "sles" | "sled" | "sle-micro" | "sl-micro" => Family::Suse,
        id if id.starts_with("opensuse") => Family::Suse,
        "debian" | "ubuntu" => Family::Debian,
        "arch" | "archlinux" | "steamos" => Family::Arch,
        "alpine" => Family::Alpine,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    /// `None` — NixOS, macOS або невідома система без знайомого менеджера.
    pub family: Option<Family>,
    /// Системні пакунки змінюються атомарно або змонтовані лише для читання.
    pub immutable: bool,
    /// Безперервні випуски (openSUSE Tumbleweed, Slowroll).
    pub rolling: bool,
    root: PathBuf,
}

impl Default for Platform {
    fn default() -> Self {
        Platform {
            family: None,
            immutable: false,
            rolling: false,
            root: PathBuf::from("/"),
        }
    }
}

impl Platform {
    /// Цей комп'ютер. `UPM_HOST_ROOT` підміняє корінь системи (для перевірок);
    /// впливає лише на те, які джерела вважаються доступними.
    pub fn detect(have: impl Fn(&str) -> bool) -> Self {
        let root = std::env::var_os("UPM_HOST_ROOT")
            .filter(|root| !root.is_empty())
            .map_or_else(|| PathBuf::from("/"), PathBuf::from);
        Self::probe(&root, std::env::consts::OS, have)
    }

    /// `root` — корінь системи, `os` — як у `std::env::consts::OS`,
    /// `have` — чи є така програма в `PATH`.
    pub fn probe(root: &Path, os: &str, have: impl Fn(&str) -> bool) -> Self {
        let mut platform = Platform {
            root: root.to_path_buf(),
            ..Platform::default()
        };
        if os != "linux" {
            return platform;
        }
        let release = ["etc/os-release", "usr/lib/os-release"]
            .iter()
            .find_map(|file| std::fs::read_to_string(platform.path(file)).ok())
            .unwrap_or_default();
        let value = |key: &str| {
            release
                .lines()
                .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
                .map(|value| value.trim().trim_matches(['"', '\'']).to_lowercase())
                .unwrap_or_default()
        };
        let id = value("ID");
        let similar = value("ID_LIKE");
        // Власний ID важливіший за "схожий на" — і ті в оголошеному порядку.
        let named = std::iter::once(id.as_str())
            .chain(similar.split_whitespace())
            .find_map(family_of);
        platform.family = if id == "nixos" {
            None
        } else {
            named.or_else(|| platform.family_by_database(have))
        };
        platform.rolling = id.contains("tumbleweed") || id.contains("slowroll");
        platform.immutable = platform.path("run/ostree-booted").exists()
            || id == "steamos"
            || (platform.family == Some(Family::Suse)
                && IMMUTABLE_SUSE.iter().any(|name| id.contains(name)));
        platform
    }

    /// Для невідомого дистрибутива: перше сімейство, у якого є і база
    /// пакунків, і менеджер.
    fn family_by_database(&self, have: impl Fn(&str) -> bool) -> Option<Family> {
        let candidates: [(Family, &[&str], &str); 6] = [
            (Family::Debian, &["var/lib/dpkg/status"], "apt-get"),
            (Family::Arch, &["var/lib/pacman/local"], "pacman"),
            (Family::Alpine, &["etc/apk/world"], "apk"),
            (Family::Suse, &["etc/zypp"], "zypper"),
            (Family::Rpm, &["var/lib/rpm"], "rpm"),
            (Family::Rpm, &["usr/lib/sysimage/rpm"], "rpm"),
        ];
        candidates
            .into_iter()
            .find(|(_, databases, tool)| {
                databases.iter().any(|path| self.path(path).exists()) && have(tool)
            })
            .map(|(family, _, _)| family)
    }

    /// Чи можна тут керувати пакунками цього сімейства.
    pub fn manages(&self, family: Family) -> bool {
        self.family == Some(family) && !self.immutable
    }

    /// Системний шлях відносно кореня.
    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative.trim_start_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Host;

    fn probe(os_release: &str, files: &[&str], tools: &[&str]) -> Platform {
        let mut host = Host::new(os_release);
        for file in files {
            host.file(file, "");
        }
        Platform::probe(&host.dir.join("root"), "linux", |tool| tools.contains(&tool))
    }

    fn family(os_release: &str) -> Option<Family> {
        probe(os_release, &[], &[]).family
    }

    #[test]
    fn known_distributions() {
        for (release, expected) in [
            ("ID=fedora\n", Family::Rpm),
            ("ID=\"centos\"\nID_LIKE=\"rhel fedora\"\n", Family::Rpm),
            ("ID=altlinux\n", Family::Rpm),
            ("ID=debian\n", Family::Debian),
            ("ID=ubuntu\nID_LIKE=debian\n", Family::Debian),
            ("ID=arch\n", Family::Arch),
            ("ID=alpine\n", Family::Alpine),
            ("ID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n", Family::Suse),
            ("ID=\"sles\"\n", Family::Suse),
        ] {
            assert_eq!(family(release), Some(expected), "{release}");
        }
    }

    #[test]
    fn derivative_is_recognised_through_id_like_in_order() {
        assert_eq!(
            family("ID=something\nID_LIKE=\"ubuntu debian\"\n"),
            Some(Family::Debian)
        );
        assert_eq!(family("ID=odd\nID_LIKE='arch debian'\n"), Some(Family::Arch));
    }

    #[test]
    fn tumbleweed_is_a_rolling_suse() {
        let release = include_str!("../tests/fixtures/zypper-tumbleweed/os-release.txt");
        let platform = probe(release, &[], &[]);
        assert_eq!(platform.family, Some(Family::Suse));
        assert!(platform.rolling && !platform.immutable);
        assert!(probe("ID=opensuse-slowroll\n", &[], &[]).rolling);
        assert!(!probe("ID=opensuse-leap\n", &[], &[]).rolling);
    }

    #[test]
    fn foreign_tools_never_change_the_family() {
        let platform = probe("ID=fedora\n", &["var/lib/dpkg/status"], &["apt-get", "dpkg"]);
        assert_eq!(platform.family, Some(Family::Rpm));
        // Debian без apt-get лишається Debian: чужого не підставляємо.
        let platform = probe("ID=debian\n", &["var/lib/pacman/local/x"], &["pacman"]);
        assert_eq!(platform.family, Some(Family::Debian));
    }

    #[test]
    fn unknown_distribution_picks_one_family_by_database_and_tool() {
        let files = ["var/lib/pacman/local/x", "var/lib/rpm/x"];
        let tools = ["pacman", "rpm"];
        assert_eq!(probe("ID=mystery\n", &files, &tools).family, Some(Family::Arch));
        // Debian перевіряється першим.
        let files = ["var/lib/dpkg/status", "var/lib/pacman/local/x"];
        let tools = ["apt-get", "pacman"];
        assert_eq!(probe("ID=mystery\n", &files, &tools).family, Some(Family::Debian));
        // Лише база без програми не рахується.
        assert_eq!(probe("ID=mystery\n", &["etc/apk/world"], &[]).family, None);
        assert_eq!(
            probe("ID=mystery\n", &["etc/apk/world"], &["apk"]).family,
            Some(Family::Alpine)
        );
        assert_eq!(
            probe("", &["etc/zypp/x", "var/lib/rpm/x"], &["zypper", "rpm"]).family,
            Some(Family::Suse)
        );
        assert_eq!(
            probe("", &["usr/lib/sysimage/rpm/x"], &["rpm"]).family,
            Some(Family::Rpm)
        );
        assert_eq!(probe("ID=mystery\n", &[], &[]).family, None);
    }

    #[test]
    fn os_release_may_live_in_usr_lib_or_be_missing() {
        let mut host = Host::new("");
        let root = host.dir.join("root");
        std::fs::remove_file(root.join("etc/os-release")).unwrap();
        assert_eq!(Platform::probe(&root, "linux", |_| false).family, None);
        host.file("usr/lib/os-release", "ID=alpine\n");
        assert_eq!(
            Platform::probe(&root, "linux", |_| false).family,
            Some(Family::Alpine)
        );
    }

    #[test]
    fn nixos_and_macos_have_no_native_family() {
        let files = ["var/lib/dpkg/status"];
        assert_eq!(probe("ID=nixos\n", &files, &["apt-get"]).family, None);
        let host = Host::new("ID=fedora\n");
        let mac = Platform::probe(&host.dir.join("root"), "macos", |_| true);
        assert_eq!(mac.family, None);
    }

    #[test]
    fn immutable_hosts_are_marked() {
        let release = include_str!("../tests/fixtures/immutable/silverblue-os-release.txt");
        let silverblue = probe(release, &["run/ostree-booted"], &[]);
        assert_eq!(silverblue.family, Some(Family::Rpm));
        assert!(silverblue.immutable);
        assert!(!probe(release, &[], &[]).immutable);

        let steamos = probe(include_str!("../tests/fixtures/steamos/os-release.txt"), &[], &[]);
        assert_eq!(steamos.family, Some(Family::Arch));
        assert!(steamos.immutable);

        let microos = include_str!("../tests/fixtures/immutable/microos-os-release.txt");
        for release in [
            microos,
            "ID=opensuse-aeon\n",
            "ID=opensuse-leap-micro\n",
            "ID=sl-micro\n",
            "ID=sle-micro\n",
        ] {
            let platform = probe(release, &[], &[]);
            assert_eq!(platform.family, Some(Family::Suse), "{release}");
            assert!(platform.immutable, "{release}");
        }
        assert!(!probe("ID=arch\n", &[], &[]).immutable);
    }
}
