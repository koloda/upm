use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.upm.gui").qml_files(["qml/Main.qml", "qml/MainView.qml"]),
    )
    .files(["src/backend.rs"])
    .build();
}
