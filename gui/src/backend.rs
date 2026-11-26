//! Об'єкт `Backend`, який бачить QML. Лише тримає стан екрана й запускає
//! роботу у фоновому потоці; уся логіка — в [`crate::jobs`] та ядрі.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::QString;
use upm_common::catalog::{Catalog, Filter};
use upm_common::context::{Context, find_askpass};
use upm_common::report::Reporter;
use upm_common::sources;

use crate::jobs::{self, Job, Outcome};
use crate::view;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, sources_json)]
        #[qproperty(QString, rows_json)]
        #[qproperty(QString, current_source)]
        #[qproperty(bool, only_upgradable)]
        #[qproperty(QString, search)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, upgrading)]
        #[qproperty(QString, status)]
        #[qproperty(QString, log)]
        type Backend = super::BackendRust;

        /// "" — усі джерела.
        #[qinvokable]
        fn select_source(self: Pin<&mut Backend>, id: &QString);

        #[qinvokable]
        fn set_filter(self: Pin<&mut Backend>, only_upgradable: bool, search: &QString);

        /// Перевірити оновлення (`update`).
        #[qinvokable]
        fn refresh(self: Pin<&mut Backend>);

        /// Опис того, що зробить `upgrade`, для вікна підтвердження.
        #[qinvokable]
        fn plan_upgrade(self: &Backend, selected: &QString) -> QString;

        #[qinvokable]
        fn upgrade(self: Pin<&mut Backend>, selected: &QString);
    }

    impl cxx_qt::Threading for Backend {}
    impl cxx_qt::Initialize for Backend {}
}

use qobject::Backend;

/// Скільки тексту тримати в журналі.
const LOG_LIMIT: usize = 100_000;

#[derive(Default)]
pub struct BackendRust {
    sources_json: QString,
    rows_json: QString,
    current_source: QString,
    only_upgradable: bool,
    search: QString,
    busy: bool,
    upgrading: bool,
    status: QString,
    log: QString,

    catalog: Arc<Mutex<Catalog>>,
    log_text: String,
}

/// Повідомлення ядра потрапляють у журнал вікна.
struct Journal(CxxQtThread<Backend>);

impl Journal {
    fn send(&self, text: String) {
        // Помилка означає, що вікна вже немає.
        let _ = self.0.queue(move |backend| backend.append_log(&text));
    }
}

impl Reporter for Journal {
    fn info(&self, text: &str) {
        self.send(format!("  {text}\n"));
    }
    fn warn(&self, text: &str) {
        self.send(format!("  ! {text}\n"));
    }
    fn diagnostic(&self, text: &str) {
        self.send(format!("  ! {text}\n"));
    }
    fn log(&self, text: &str) {
        self.send(text.to_string());
    }
    fn command(&self, line: &str, _side: bool) {
        self.send(format!("+ {line}\n"));
    }
    fn failure(&self, problem: &str) {
        self.send(format!("  {problem}\n"));
    }
    fn captures_output(&self) -> bool {
        true
    }
}

/// `--dry-run`: Upgrade лише показує команди.
fn dry_run() -> bool {
    std::env::args().any(|arg| arg == "-n" || arg == "--dry-run")
}

impl cxx_qt::Initialize for Backend {
    fn initialize(self: Pin<&mut Self>) {
        self.start(Job::Load, "Читаю список установленого…");
    }
}

impl Backend {
    fn selected_source(&self) -> Option<String> {
        Some(self.current_source().to_string()).filter(|id| !id.is_empty())
    }

    /// Оновлює бічну панель і таблицю за каталогом та фільтрами.
    fn publish(mut self: Pin<&mut Self>) {
        let source = self.selected_source();
        let search = self.search().to_string();
        let filter = Filter {
            source: source.as_deref(),
            only_upgradable: *self.only_upgradable(),
            search: &search,
        };
        let (sidebar, rows) = {
            let catalog = jobs::lock(&self.rust().catalog);
            (view::sources(&catalog), view::rows(&catalog, &filter))
        };
        self.as_mut().set_sources_json(QString::from(&sidebar));
        self.set_rows_json(QString::from(&rows));
    }

    fn append_log(mut self: Pin<&mut Self>, text: &str) {
        let log = {
            let mut rust = self.as_mut().rust_mut();
            rust.log_text.push_str(text);
            if rust.log_text.len() > LOG_LIMIT {
                let mut cut = rust.log_text.len() - LOG_LIMIT / 2;
                while !rust.log_text.is_char_boundary(cut) {
                    cut += 1;
                }
                rust.log_text.drain(..cut);
            }
            QString::from(&rust.log_text)
        };
        self.set_log(log);
    }

    /// Запускає роботу у фоні. Одночасно виконується лише одна.
    fn start(mut self: Pin<&mut Self>, job: Job, status: &str) {
        if *self.busy() {
            return;
        }
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            self.set_status(QString::from("Не задано змінну HOME"));
            return;
        };
        self.as_mut().set_busy(true);
        self.as_mut()
            .set_upgrading(matches!(job, Job::Upgrade { .. }));
        self.as_mut().set_status(QString::from(status));

        let screen = self.qt_thread();
        let catalog = Arc::clone(&self.rust().catalog);
        let dry = dry_run();
        std::thread::spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let journal = Box::new(Journal(screen.clone()));
                // Підтвердження вже було у вікні, тому "yes".
                let mut ctx = Context::new(home, dry, true, journal);
                ctx.askpass = find_askpass();
                let sources = sources::all();
                jobs::run(job, &mut ctx, &sources, &catalog, &|| {
                    let _ = screen.queue(|backend| backend.publish());
                })
            }))
            .unwrap_or_else(|_| Outcome {
                status: "Внутрішня помилка, деталі в терміналі".to_string(),
                only_upgradable: None,
            });
            let _ = screen.queue(move |backend| backend.finish(outcome));
        });
    }

    fn finish(mut self: Pin<&mut Self>, outcome: Outcome) {
        if let Some(only) = outcome.only_upgradable {
            self.as_mut().set_only_upgradable(only);
        }
        self.as_mut().set_status(QString::from(&outcome.status));
        self.as_mut().set_upgrading(false);
        self.as_mut().set_busy(false);
        self.publish();
    }

    pub fn select_source(mut self: Pin<&mut Self>, id: &QString) {
        self.as_mut().set_current_source(id.clone());
        self.publish();
    }

    pub fn set_filter(mut self: Pin<&mut Self>, only_upgradable: bool, search: &QString) {
        self.as_mut().set_only_upgradable(only_upgradable);
        self.as_mut().set_search(search.clone());
        self.publish();
    }

    pub fn refresh(self: Pin<&mut Self>) {
        let source = self.selected_source();
        self.start(Job::Check { source }, "Перевіряю оновлення…");
    }

    pub fn plan_upgrade(&self, selected: &QString) -> QString {
        let Some(selected) = view::selection(&selected.to_string()) else {
            return QString::from(r#"{"runnable":false,"title":"Помилка вибору","lines":[]}"#);
        };
        let source = self.selected_source();
        let catalog = jobs::lock(&self.rust().catalog);
        let plan = catalog.upgrade_plan(source.as_deref(), &selected);
        QString::from(&view::confirmation(&plan))
    }

    pub fn upgrade(self: Pin<&mut Self>, selected: &QString) {
        let Some(selected) = view::selection(&selected.to_string()) else {
            return;
        };
        let source = self.selected_source();
        self.start(Job::Upgrade { source, selected }, "Оновлюю…");
    }
}
