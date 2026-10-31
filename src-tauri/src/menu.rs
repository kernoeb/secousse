//! The macOS menu bar in the UI language. Tauri's default menu has fixed English labels.

use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID};
use tauri::{AppHandle, Runtime};

struct Labels {
    about: &'static str,
    services: &'static str,
    hide: &'static str,
    hide_others: &'static str,
    quit: &'static str,
    file: &'static str,
    close_window: &'static str,
    edit: &'static str,
    undo: &'static str,
    redo: &'static str,
    cut: &'static str,
    copy: &'static str,
    paste: &'static str,
    select_all: &'static str,
    view: &'static str,
    fullscreen: &'static str,
    window: &'static str,
    minimize: &'static str,
    zoom: &'static str,
    help: &'static str,
}

/// Wording follows the menus of Apple's own apps in each language. `{}` is the app name.
fn labels(language: &str) -> Labels {
    match language {
        "fr" => Labels {
            about: "À propos de {}",
            services: "Services",
            hide: "Masquer {}",
            hide_others: "Masquer les autres",
            quit: "Quitter {}",
            file: "Fichier",
            close_window: "Fermer la fenêtre",
            edit: "Édition",
            undo: "Annuler",
            redo: "Rétablir",
            cut: "Couper",
            copy: "Copier",
            paste: "Coller",
            select_all: "Tout sélectionner",
            view: "Présentation",
            fullscreen: "Plein écran",
            window: "Fenêtre",
            minimize: "Réduire",
            zoom: "Zoom",
            help: "Aide",
        },
        "es" => Labels {
            about: "Acerca de {}",
            services: "Servicios",
            hide: "Ocultar {}",
            hide_others: "Ocultar otros",
            quit: "Salir de {}",
            file: "Archivo",
            close_window: "Cerrar ventana",
            edit: "Edición",
            undo: "Deshacer",
            redo: "Rehacer",
            cut: "Cortar",
            copy: "Copiar",
            paste: "Pegar",
            select_all: "Seleccionar todo",
            view: "Visualización",
            fullscreen: "Pantalla completa",
            window: "Ventana",
            minimize: "Minimizar",
            zoom: "Zoom",
            help: "Ayuda",
        },
        _ => Labels {
            about: "About {}",
            services: "Services",
            hide: "Hide {}",
            hide_others: "Hide Others",
            quit: "Quit {}",
            file: "File",
            close_window: "Close Window",
            edit: "Edit",
            undo: "Undo",
            redo: "Redo",
            cut: "Cut",
            copy: "Copy",
            paste: "Paste",
            select_all: "Select All",
            view: "View",
            fullscreen: "Toggle Full Screen",
            window: "Window",
            minimize: "Minimize",
            zoom: "Zoom",
            help: "Help",
        },
    }
}

/// Same items as `Menu::default` on macOS, with translated labels.
pub fn build<R: Runtime>(app: &AppHandle<R>, language: &str) -> tauri::Result<Menu<R>> {
    let l = labels(language);
    let name = app.package_info().name.clone();
    let named = |label: &str| label.replace("{}", &name);
    let config = app.config();
    let about_metadata = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        copyright: config.bundle.copyright.clone(),
        authors: config.bundle.publisher.clone().map(|p| vec![p]),
        ..Default::default()
    };

    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                &name,
                true,
                &[
                    &PredefinedMenuItem::about(app, Some(&named(l.about)), Some(about_metadata))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, Some(l.services))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, Some(&named(l.hide)))?,
                    &PredefinedMenuItem::hide_others(app, Some(l.hide_others))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, Some(&named(l.quit)))?,
                ],
            )?,
            &Submenu::with_items(app, l.file, true, &[&PredefinedMenuItem::close_window(app, Some(l.close_window))?])?,
            &Submenu::with_items(
                app,
                l.edit,
                true,
                &[
                    &PredefinedMenuItem::undo(app, Some(l.undo))?,
                    &PredefinedMenuItem::redo(app, Some(l.redo))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::cut(app, Some(l.cut))?,
                    &PredefinedMenuItem::copy(app, Some(l.copy))?,
                    &PredefinedMenuItem::paste(app, Some(l.paste))?,
                    &PredefinedMenuItem::select_all(app, Some(l.select_all))?,
                ],
            )?,
            &Submenu::with_items(app, l.view, true, &[&PredefinedMenuItem::fullscreen(app, Some(l.fullscreen))?])?,
            // These ids make macOS treat the menus as the Window and Help menus.
            &Submenu::with_id_and_items(
                app,
                WINDOW_SUBMENU_ID,
                l.window,
                true,
                &[
                    &PredefinedMenuItem::minimize(app, Some(l.minimize))?,
                    &PredefinedMenuItem::maximize(app, Some(l.zoom))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::close_window(app, Some(l.close_window))?,
                ],
            )?,
            &Submenu::with_id_and_items(app, HELP_SUBMENU_ID, l.help, true, &[])?,
        ],
    )
}
