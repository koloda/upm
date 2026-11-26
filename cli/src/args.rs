//! Розбір командного рядка.

use upm_common::scope::{Scope, Target};
use upm_common::sources::{self, canonical};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Action {
    #[default]
    Help,
    List,
    Update,
    Upgrade,
}

#[derive(Debug, Default)]
pub struct Request {
    pub action: Action,
    pub yes: bool,
    pub dry: bool,
    /// list: разом із залежностями.
    pub all: bool,
    /// Які джерела й пакунки вибрано.
    pub scope: Scope,
}

pub fn parse(args: &[String]) -> Result<Request, String> {
    let mut request = Request::default();
    let mut action = None;
    // Параметри upgrade без команди (як у попередній версії скрипта).
    let mut legacy = false;
    let mut args = args.iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-y" | "--yes" => (request.yes, legacy) = (true, true),
            "-n" | "--dry-run" => (request.dry, legacy) = (true, true),
            "-a" | "--all" => request.all = true,
            "--firmware" => (request.scope.firmware, legacy) = (true, true),
            flag @ ("--only" | "--skip") => {
                let list = args
                    .next()
                    .ok_or_else(|| format!("Після {flag} потрібен список джерел"))?;
                for word in list.split([',', ' ']).filter(|word| !word.is_empty()) {
                    let id = canonical(word).ok_or_else(|| format!("Невідоме джерело: {word}"))?;
                    if flag == "--only" {
                        request.scope.firmware |= id == "firmware";
                        request.scope.only.push(id);
                    } else {
                        request.scope.skip.push(id);
                    }
                }
                legacy = true;
            }
            "-h" | "--help" => {
                request.action = Action::Help;
                return Ok(request);
            }
            other if other.starts_with('-') => {
                return Err(format!("Невідомий параметр: {other}"));
            }
            word if action.is_none() => {
                action = Some(match word {
                    "list" => Action::List,
                    "update" => Action::Update,
                    "upgrade" => Action::Upgrade,
                    "help" => Action::Help,
                    _ => return Err(format!("Невідома команда: {word}")),
                });
            }
            word => {
                if let Some(id) = canonical(word) {
                    request.scope.firmware |= id == "firmware";
                    request.scope.sources.push(id);
                    continue;
                }
                let qualified = word
                    .split_once(':')
                    .and_then(|(source, query)| Some((canonical(source)?, query)));
                let (source, query) = match qualified {
                    Some((id, query)) => (Some(id), query),
                    None => (None, word),
                };
                request.scope.firmware |= source == Some("firmware");
                request.scope.targets.push(Target {
                    source,
                    query: query.to_string(),
                });
            }
        }
    }

    request.action = action.unwrap_or(if legacy {
        Action::Upgrade
    } else {
        Action::Help
    });
    Ok(request)
}

pub fn usage(program: &str) -> String {
    let all = sources::NAMES.join(" ");
    format!(
        "Використання: {program} <команда> [параметри] [джерело | пакунок | джерело:пакунок ...]

Команди:
  list       показати встановлене (пакунок шукається за частиною назви)
  update     оновити списки репозиторіїв і показати доступні оновлення
  upgrade    оновити все або тільки вказане (пакунок — за точною назвою)
  help       ця довідка

Параметри:
  -a, --all          list: разом із залежностями
  -y, --yes          upgrade: не перепитувати підтвердження
  -n, --dry-run      upgrade: лише показати, що буде запущено
      --only СПИСОК  тільки вказані джерела, через кому
      --skip СПИСОК  усі, крім вказаних
      --firmware     також прошивки (fwupd); без цього вони не чіпаються
  -h, --help         ця довідка

Джерела: {all}
Показуються лише ті, що є на цьому комп'ютері; пакунки дистрибутива — тільки
для вашої системи.
Синоніми: node/nodejs=npm, py/pip=python, cargo=rust, golang=go, homebrew=brew,
          appimages=appimage, sdk=sdkman, deb/dpkg=apt,
          rpm — RPM-джерело цього комп'ютера (dnf, yum, zypper чи rpm)

Приклади:
  {program} list                    # усе встановлене
  {program} list node               # тільки глобальні пакунки Node.js
  {program} list firefox            # де і яка версія
  {program} list dnf --all          # усі RPM, із залежностями
  {program} update                  # що можна оновити
  {program} update python go
  {program} upgrade                 # оновити все
  {program} upgrade lazygit codex   # тільки ці
  {program} upgrade dnf:firefox     # саме з цього джерела
  {program} upgrade -y --skip android,sdkman"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(line: &str) -> Request {
        let args: Vec<String> = line.split_whitespace().map(String::from).collect();
        parse(&args).unwrap()
    }

    #[test]
    fn no_arguments_show_help() {
        assert_eq!(run("").action, Action::Help);
    }

    #[test]
    fn options_without_command_mean_upgrade() {
        assert_eq!(run("--only npm -n").action, Action::Upgrade);
    }

    #[test]
    fn words_split_into_sources_and_packages() {
        let request = run("upgrade node dnf:firefox lazygit foo:bar");
        assert_eq!(request.scope.sources, ["npm"]);
        let targets: Vec<_> = request
            .scope
            .targets
            .iter()
            .map(|t| (t.source, t.query.as_str()))
            .collect();
        // "foo" — не джерело, тож "foo:bar" є назвою пакунка.
        assert_eq!(
            targets,
            [(Some("dnf"), "firefox"), (None, "lazygit"), (None, "foo:bar")]
        );
    }

    #[test]
    fn firmware_needs_to_be_asked_for() {
        assert!(!run("upgrade").scope.in_scope("firmware"));
        assert!(run("upgrade --firmware").scope.in_scope("firmware"));
        assert!(run("upgrade firmware:SSD").scope.in_scope("firmware"));
        assert!(run("list --only firmware").scope.in_scope("firmware"));
    }

    #[test]
    fn rpm_and_debian_words_are_sources() {
        // Яке саме RPM-джерело — вирішує комп'ютер уже після розбору.
        let request = run("upgrade rpm deb:firefox");
        assert_eq!(request.scope.sources, ["rpm"]);
        assert_eq!(request.scope.targets[0].source, Some("apt"));
    }

    #[test]
    fn help_names_every_source_and_synonym() {
        let help = usage("upm");
        for word in sources::NAMES {
            assert!(help.contains(word), "{word}");
        }
        assert!(help.contains("deb/dpkg=apt"), "{help}");
        assert!(!help.contains("rpm=dnf"), "{help}");
        assert!(help.contains("rpm — RPM-джерело цього комп'ютера"), "{help}");
    }
}
