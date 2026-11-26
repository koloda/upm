//! Оточення запуску та виконання зовнішніх команд.

use std::ffi::OsString;
use std::io::{IsTerminal, Read};
use std::os::fd::AsFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::report::Reporter;

/// Зовнішня команда: програма, аргументи та додаткові змінні середовища.
#[derive(Clone)]
pub struct Cmd {
    argv: Vec<String>,
    envs: Vec<(String, String)>,
    show_stderr: bool,
    keep_stdin: bool,
}

impl Cmd {
    pub fn new<I, S>(argv: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Cmd {
            argv: argv.into_iter().map(Into::into).collect(),
            envs: Vec::new(),
            show_stderr: true,
            keep_stdin: false,
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.argv.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.envs.push((key.to_string(), value.to_string()));
        self
    }

    /// Сховати stderr під час збирання виводу.
    pub fn quiet(mut self) -> Self {
        self.show_stderr = false;
        self
    }

    /// Залишити термінал на stdin (наприклад, для запиту пароля).
    pub fn interactive(mut self) -> Self {
        self.keep_stdin = true;
        self
    }

    pub fn display(&self) -> String {
        self.argv.join(" ")
    }

    /// Змушує sudo запитувати пароль через віконну програму `helper`.
    fn use_askpass(&mut self, helper: &Path) {
        let may_prompt = self.argv[0] == "sudo" && !self.argv.iter().any(|arg| arg == "-n");
        if may_prompt {
            self.argv.insert(1, "-A".to_string());
            self.envs
                .push(("SUDO_ASKPASS".to_string(), helper.display().to_string()));
        }
    }
}

/// Результат команди, вивід якої ми прочитали.
pub struct Captured {
    /// `None`, якщо команду не вдалося запустити або її вбито сигналом.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Captured {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }
}

pub struct Context {
    pub home: PathBuf,
    path: OsString,
    pub dry: bool,
    pub yes: bool,
    pub failed_commands: Vec<String>,
    pub notes: Vec<String>,
    /// Віконна програма для пароля sudo, коли термінала немає.
    pub askpass: Option<PathBuf>,
    interrupted: Arc<AtomicBool>,
    reporter: Box<dyn Reporter>,
}

impl Context {
    pub fn new(home: PathBuf, dry: bool, yes: bool, reporter: Box<dyn Reporter>) -> Self {
        // Щоб працювати однаково з fish, bash, cron чи ярлика.
        let mut dirs: Vec<PathBuf> =
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
        let extra = [
            home.join(".local/bin"),
            PathBuf::from("/home/linuxbrew/.linuxbrew/bin"),
            home.join("go/bin"),
            home.join(".composer/vendor/bin"),
            home.join(".opencode/bin"),
            home.join(".mimocode/bin"),
            home.join(".cargo/bin"),
            home.join(".bun/bin"),
        ];
        for dir in extra {
            if dir.is_dir() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        let path = std::env::join_paths(dirs).unwrap_or_default();

        Context {
            home,
            path,
            dry,
            yes,
            failed_commands: Vec::new(),
            notes: Vec::new(),
            askpass: None,
            interrupted: Arc::new(AtomicBool::new(false)),
            reporter,
        }
    }

    /// Прапорець переривання: інтерфейс виставляє його по Ctrl+C чи кнопці
    /// «Скасувати», і нові команди більше не запускаються.
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupted)
    }

    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Relaxed)
    }

    pub fn info(&self, text: &str) {
        self.reporter.info(text);
    }

    pub fn warn(&self, text: &str) {
        self.reporter.warn(text);
    }

    pub fn diagnostic(&self, text: &str) {
        self.reporter.diagnostic(text);
    }

    pub fn log(&self, text: &str) {
        self.reporter.log(text);
    }

    /// Шлях відносно домашньої директорії.
    pub fn in_home(&self, relative: &str) -> PathBuf {
        self.home.join(relative)
    }

    pub fn have(&self, program: &str) -> bool {
        std::env::split_paths(&self.path).any(|dir| is_executable(&dir.join(program)))
    }

    /// Чи можуть сторонні програми запитувати щось у користувача.
    pub fn interactive(&self) -> bool {
        !self.reporter.captures_output() && std::io::stdin().is_terminal()
    }

    fn command(&self, cmd: &Cmd) -> Command {
        let mut cmd = cmd.clone();
        if let Some(helper) = &self.askpass {
            cmd.use_askpass(helper);
        }
        let mut command = Command::new(&cmd.argv[0]);
        command
            .args(&cmd.argv[1..])
            .env("PATH", &self.path)
            .current_dir(&self.home);
        for (key, value) in &cmd.envs {
            command.env(key, value);
        }
        command
    }

    /// Виконує запит і повертає вивід. Працює і в режимі --dry-run.
    pub fn capture(&self, cmd: &Cmd) -> Captured {
        // Після Ctrl+C нові команди не запускаються.
        if self.interrupted() {
            return Captured {
                code: None,
                stdout: String::new(),
                stderr: String::new(),
            };
        }
        let mut command = self.command(cmd);
        let captures = self.reporter.captures_output();
        command.stdin(if cmd.keep_stdin && !captures {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        command.stderr(if cmd.show_stderr && !captures {
            Stdio::inherit()
        } else {
            Stdio::piped()
        });
        match command.stdout(Stdio::piped()).output() {
            Ok(result) => {
                let stderr = String::from_utf8_lossy(&result.stderr).into_owned();
                if captures && cmd.show_stderr && !stderr.is_empty() {
                    self.reporter.log(&stderr);
                }
                Captured {
                    code: result.status.code(),
                    stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
                    stderr,
                }
            }
            Err(error) => {
                self.diagnostic(&format!("{}: {error}", cmd.argv[0]));
                Captured {
                    code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                }
            }
        }
    }

    /// Вивід успішної команди або `None`.
    pub fn read(&self, cmd: &Cmd) -> Option<String> {
        let result = self.capture(cmd);
        result.ok().then_some(result.stdout)
    }

    /// Виконує команду (у --dry-run лише показує). Невдачі запам'ятовує.
    pub fn run(&mut self, cmd: &Cmd) -> bool {
        if self.interrupted() {
            return false;
        }
        self.reporter.command(&cmd.display(), false);
        if self.dry {
            return true;
        }
        let status = if self.reporter.captures_output() {
            self.stream(cmd)
        } else {
            self.command(cmd).status()
        };
        self.record(cmd, status)
    }

    /// Як `run`, але весь вивід іде в stderr, щоб не змішуватися з таблицями.
    pub fn refresh(&mut self, cmd: &Cmd) -> bool {
        if self.interrupted() {
            return false;
        }
        self.reporter.command(&cmd.display(), true);
        if self.dry {
            return true;
        }
        let status = if self.reporter.captures_output() {
            self.stream(cmd)
        } else {
            let mut command = self.command(cmd);
            if let Ok(stderr) = std::io::stderr().as_fd().try_clone_to_owned() {
                command.stdout(Stdio::from(stderr));
            }
            command.status()
        };
        self.record(cmd, status)
    }

    /// Виконує команду, передаючи її вивід у `Reporter::log` рядок за рядком.
    fn stream(&self, cmd: &Cmd) -> std::io::Result<ExitStatus> {
        let mut child = self
            .command(cmd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let (sender, lines) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            forward_lines(stdout, sender.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            forward_lines(stderr, sender.clone());
        }
        drop(sender);

        let mut status = None;
        loop {
            match lines.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => self.reporter.log(&line),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                // Фоновий нащадок може тримати канал відкритим і після завершення команди.
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if status.is_some() {
                        break;
                    }
                    status = child.try_wait()?;
                }
            }
        }
        match status {
            Some(status) => Ok(status),
            None => child.wait(),
        }
    }

    fn record(&mut self, cmd: &Cmd, status: std::io::Result<ExitStatus>) -> bool {
        let problem = match status {
            Ok(status) if status.success() => return true,
            Ok(status) => match status.code() {
                Some(code) => format!("команда завершилась з кодом {code}"),
                None => "команду перервано".to_string(),
            },
            Err(error) => format!("не вдалося запустити: {error}"),
        };
        self.failed_commands.push(cmd.display());
        self.reporter.failure(&problem);
        false
    }

    /// `sudo` лише тоді, коли поточний користувач не може писати в каталог.
    pub fn run_in_dir(&mut self, dir: &Path, cmd: Cmd) -> bool {
        if self.writable(dir) {
            self.run(&cmd)
        } else {
            let mut elevated = Cmd::new(["sudo"]).args(cmd.argv);
            elevated.envs = cmd.envs;
            self.run(&elevated)
        }
    }

    fn writable(&self, dir: &Path) -> bool {
        let test = Cmd::new(["test", "-w"]).args([dir.display().to_string()]);
        self.capture(&test).ok()
    }

    /// Чи належить файл пакунку RPM (тоді його оновлює DNF).
    pub fn rpm_owned(&self, path: &Path) -> bool {
        if !self.have("rpm") {
            return false;
        }
        let query = Cmd::new(["rpm", "-qf"])
            .args([path.display().to_string()])
            .quiet();
        self.capture(&query).ok()
    }

    pub fn note(&mut self, text: String) {
        self.warn(&text);
        self.notes.push(text);
    }
}

/// Читає потік і надсилає рядки; "\r" (смуги прогресу) теж кінець рядка.
fn forward_lines(mut stream: impl Read + Send + 'static, sender: mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let mut pending = Vec::new();
        let mut buffer = [0; 4096];
        let send = |bytes: &[u8]| {
            let line = format!("{}\n", String::from_utf8_lossy(bytes));
            sender.send(line).is_ok()
        };
        while let Ok(count @ 1..) = stream.read(&mut buffer) {
            for &byte in &buffer[..count] {
                if byte != b'\n' && byte != b'\r' {
                    pending.push(byte);
                } else if !pending.is_empty() {
                    if !send(&pending) {
                        return;
                    }
                    pending.clear();
                }
            }
        }
        if !pending.is_empty() {
            send(&pending);
        }
    });
}

/// Віконна програма для запиту пароля, якщо така є.
pub fn find_askpass() -> Option<PathBuf> {
    std::env::var_os("SUDO_ASKPASS")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            [
                "/usr/bin/ksshaskpass",
                "/usr/libexec/openssh/ssh-askpass",
                "/usr/libexec/openssh/gnome-ssh-askpass",
            ]
            .map(PathBuf::from),
        )
        .find(|path| is_executable(path))
}

pub fn is_executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Імена в каталозі за алфавітом, без прихованих (як `*` в оболонці).
pub fn list_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    paths
}

pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Clone, Default)]
    struct Window(Arc<Mutex<String>>);

    impl Reporter for Window {
        fn info(&self, _: &str) {}
        fn warn(&self, _: &str) {}
        fn diagnostic(&self, _: &str) {}
        fn log(&self, text: &str) {
            self.0.lock().unwrap().push_str(text);
        }
        fn command(&self, _: &str, _: bool) {}
        fn failure(&self, _: &str) {}
        fn captures_output(&self) -> bool {
            true
        }
    }

    fn context() -> (Context, Window) {
        let window = Window::default();
        let ctx = Context::new("/".into(), false, true, Box::new(window.clone()));
        (ctx, window)
    }

    fn shell(script: &str) -> Cmd {
        Cmd::new(["sh", "-c", script])
    }

    #[test]
    fn captured_run_forwards_both_streams() {
        let (mut ctx, window) = context();
        assert!(ctx.run(&shell("echo out; echo err >&2; printf 'a\\rb\\n'")));
        let log = window.0.lock().unwrap().clone();
        for line in ["out\n", "err\n", "a\n", "b\n"] {
            assert!(log.contains(line), "{log:?} без {line:?}");
        }
    }

    #[test]
    fn captured_commands_never_wait_for_input() {
        let (mut ctx, window) = context();
        assert!(ctx.run(&shell("read answer || echo eof").interactive()));
        assert!(window.0.lock().unwrap().contains("eof"));
        assert!(!ctx.interactive());
    }

    #[test]
    fn captured_query_sends_stderr_to_reporter() {
        let (ctx, window) = context();
        let result = ctx.capture(&shell("echo data; echo problem >&2"));
        assert_eq!(result.stdout, "data\n");
        assert!(window.0.lock().unwrap().contains("problem"));
    }

    #[test]
    fn captured_failure_is_recorded() {
        let (mut ctx, _) = context();
        assert!(!ctx.run(&shell("exit 3")));
        assert_eq!(ctx.failed_commands.len(), 1);
    }

    #[test]
    fn askpass_applies_only_to_sudo_that_may_prompt() {
        let helper = Path::new("/usr/bin/ksshaskpass");
        let with = |argv: &[&str]| {
            let mut cmd = Cmd::new(argv.iter().copied());
            cmd.use_askpass(helper);
            (cmd.display(), cmd.envs.len())
        };
        assert_eq!(with(&["sudo", "dnf", "upgrade"]), ("sudo -A dnf upgrade".into(), 1));
        assert_eq!(with(&["sudo", "-n", "-v"]), ("sudo -n -v".into(), 0));
        assert_eq!(with(&["npm", "install"]), ("npm install".into(), 0));
    }
}
