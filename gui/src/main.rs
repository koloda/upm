//! `upm-gui` — вікно на QML над ядром upm-common.

mod backend;
mod jobs;
mod view;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};

fn main() {
    if upm_common::running_as_root() {
        eprintln!("Не запускайте через sudo: пароль UPM запитає сам, коли буде потрібно.");
        std::process::exit(1);
    }

    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/io/upm/gui/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
