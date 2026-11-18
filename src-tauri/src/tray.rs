// The tray of the panel: the window can be closed and the program stays here,
// with the switch of the overlay and the way out.

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

pub struct TrayState {
    pub hud_item: MenuItem<Wry>,
    pub show_item: MenuItem<Wry>,
    pub settings_item: MenuItem<Wry>,
    pub quit_item: MenuItem<Wry>,
}

/// Updates the words of the switch of the overlay after it changes.
pub fn refresh_hud_item(app: &AppHandle, enabled: bool) {
    let Some(state) = app.try_state::<TrayState>() else { return };
    let language = crate::i18n::language(&crate::config::load());
    let text = crate::i18n::t(&language, if enabled { "hud-off" } else { "hud-on" });
    let _ = state.hud_item.set_text(text);
}

/// Rewrites the whole menu in the language the settings ask for: the menu is
/// built once, when the program starts, and changing the language of the
/// window does not rebuild it.
pub fn refresh_texts(app: &AppHandle) {
    let Some(state) = app.try_state::<TrayState>() else { return };
    let settings = crate::config::load();
    let language = crate::i18n::language(&settings);
    let t = |key: &str| crate::i18n::t(&language, key);
    let _ = state.show_item.set_text(t("show"));
    let _ = state.settings_item.set_text(t("settings"));
    let _ = state.quit_item.set_text(t("quit"));
    let _ = state
        .hud_item
        .set_text(t(if settings.hud.enabled { "hud-off" } else { "hud-on" }));
}

pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let settings = crate::config::load();
    let language = crate::i18n::language(&settings);
    let t = |key: &str| crate::i18n::t(&language, key);

    let show = MenuItemBuilder::with_id("toggle", t("show")).build(app)?;
    let hud_text = if settings.hud.enabled { t("hud-off") } else { t("hud-on") };
    let hud = MenuItemBuilder::with_id("hud", hud_text).build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", t("settings")).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", t("quit")).build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[
            &show,
            &hud,
            &PredefinedMenuItem::separator(app)?,
            &settings_item,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ])
        .build()?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("Soplos Gaming Panel")
        .menu(&menu)
        // The left button opens the panel; the menu is the right button.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => crate::toggle_window(app),
            "hud" => crate::toggle_hud(app),
            "settings" => {
                crate::show_window(app);
                let _ = app.emit("show-view", "settings");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button, .. } = event {
                if button == MouseButton::Left {
                    crate::toggle_window(tray.app_handle());
                }
            }
        })
        .build(app)?;

    app.manage(TrayState { hud_item: hud, show_item: show, settings_item, quit_item: quit });
    Ok(())
}
