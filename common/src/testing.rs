//! Допоміжне для тестів: комп'ютер із заглушками програм замість справжніх.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::context::Context;
use crate::platform::Platform;
use crate::report::Reporter;

#[derive(Clone, Default)]
pub struct Journal {
    commands: Arc<Mutex<Vec<String>>>,
    messages: Arc<Mutex<Vec<String>>>,
    captures: Arc<Mutex<bool>>,
}

impl Reporter for Journal {
    fn info(&self, text: &str) {
        self.messages.lock().unwrap().push(text.to_string());
    }
    fn warn(&self, text: &str) {
        self.messages.lock().unwrap().push(text.to_string());
    }
    fn diagnostic(&self, text: &str) {
        self.messages.lock().unwrap().push(text.to_string());
    }
    fn log(&self, _: &str) {}
    fn command(&self, line: &str, _: bool) {
        self.commands.lock().unwrap().push(line.to_string());
    }
    fn failure(&self, _: &str) {}
    fn captures_output(&self) -> bool {
        *self.captures.lock().unwrap()
    }
}

/// Тимчасовий «комп'ютер»: власні `PATH`, домашня тека й корінь системи.
/// Програми, яких немає серед заглушок, для ядра не існують.
pub struct Host {
    pub dir: PathBuf,
    pub ctx: Context,
    journal: Journal,
}

impl Host {
    /// `os_release` — вміст `/etc/os-release`. Типово: термінал є, `-y` немає,
    /// режим не --dry-run.
    pub fn new(os_release: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "upm-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["bin", "home", "root/etc"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("root/etc/os-release"), os_release).unwrap();
        let journal = Journal::default();
        let mut ctx = Context::new(dir.join("home"), false, false, Box::new(journal.clone()));
        ctx.set_path(dir.join("bin"));
        ctx.terminal = true;
        ctx.platform = Platform::probe(&dir.join("root"), "linux", |_| false);
        Host { dir, ctx, journal }
    }

    /// Комп'ютер, на якому ці програми є і нічого не роблять.
    pub fn with(os_release: &str, tools: &[&str]) -> Self {
        let mut host = Host::new(os_release);
        for tool in tools {
            host.stub(tool, "exit 0");
        }
        host
    }

    /// Визначає платформу заново — після додавання системних файлів.
    pub fn reprobe(&mut self) -> &mut Self {
        self.ctx.platform = Platform::probe(&self.dir.join("root"), "linux", |_| false);
        self
    }

    /// Заглушка програми; `body` — тіло сценарію sh. Утиліти — повним шляхом.
    pub fn stub(&mut self, name: &str, body: &str) -> &mut Self {
        let path = self.dir.join("bin").join(name);
        let log = self.dir.join("calls.log");
        let script = format!(
            "#!/bin/sh\necho \"{name} $*\" >>'{}'\n{body}\n",
            log.display()
        );
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        self
    }

    /// `sudo`, що просто виконує решту аргументів.
    pub fn sudo(&mut self) -> &mut Self {
        self.stub("sudo", "exec \"$@\"")
    }

    /// Файл відносно кореня системи.
    pub fn file(&mut self, relative: &str, content: &str) -> &mut Self {
        let path = self.dir.join("root").join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
        self
    }

    /// Немає термінала: вивід перехоплює інтерфейс.
    pub fn headless(&mut self) -> &mut Self {
        *self.journal.captures.lock().unwrap() = true;
        self
    }

    /// Команди, які ядро виконало (або показало в --dry-run) як зміни.
    pub fn commands(&self) -> Vec<String> {
        self.journal.commands.lock().unwrap().clone()
    }

    /// Усе, що справді запускалося, включно із запитами.
    pub fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.join("calls.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// Повідомлення, попередження й діагностика одним текстом.
    pub fn messages(&self) -> String {
        self.journal.messages.lock().unwrap().join("\n")
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Тіло заглушки, що друкує збережений вивід справжньої програми.
pub fn print(fixture: &str) -> String {
    format!(
        "/usr/bin/cat '{}/tests/fixtures/{fixture}'",
        env!("CARGO_MANIFEST_DIR")
    )
}
