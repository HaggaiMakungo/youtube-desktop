#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]
// ^ Hide the damn console window on Windows release builds
// also: "please god don't let a random terminal pop up and traumatize the user"

// drag in the tauri shit we actually need
// Manager → get windows by label; Menu/MenuItem → tray menu items; TrayIconBuilder → the little guy in the taskbar
// Image → for the icon because apparently you need to import that separately like a psychopath
use tauri::{
    Manager,
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    image::Image,
};
// and the global shortcut plugin — without this, ctrl+shift+space is just vibes
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Builder as ShortcutBuilder};

// Windows bullshit: register youtube.com:// so clicking YouTube links opens this app instead of
// whatever the fuck the default browser is
#[cfg(target_os = "windows")]
fn register_youtube_handler() {
    use winreg::RegKey;
    use winreg::enums::*;

    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = exe_path.to_string_lossy().to_string();
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        
        // Register youtube.com protocol — shove it into the registry like a barbarian
        if let Ok((youtube_key, _)) = hkcu.create_subkey(r"Software\Classes\youtube.com") {
            let _ = youtube_key.set_value("", &"URL:YouTube");
            let _ = youtube_key.set_value("URL Protocol", &"");
            
            if let Ok((shell_key, _)) = youtube_key.create_subkey(r"shell\open\command") {
                let command = format!(r#""{}\" \"%1\""#, exe_str);
                let _ = shell_key.set_value("", &command);
            }
        }
    }
}

fn main() {
    // Fire up the URL handler before anything else has a chance to fuck it up
    #[cfg(target_os = "windows")]
    register_youtube_handler();

    // build the app — order matters here: plugins first, then setup(), then run()
    // if you rearrange this and shit breaks, that's on you
    tauri::Builder::default()
        // register the global shortcut plugin — has to go here before setup() or shortcuts won't exist yet
        .plugin(
            ShortcutBuilder::new()
                .build()
        )
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            // Tray menu — the only way to control your addiction when you hide the window from yourself
            let show       = MenuItem::with_id(app, "show",        "Show",            true, None::<&str>)?;
            let pin        = MenuItem::with_id(app, "pin",         "Always on Top",   true, None::<&str>)?;
            let play_pause = MenuItem::with_id(app, "play_pause",  "Play / Pause",    true, None::<&str>)?;
            let next       = MenuItem::with_id(app, "next",        "Next Video",      true, None::<&str>)?;
            let quit       = MenuItem::with_id(app, "quit",        "Quit",            true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pin, &play_pause, &next, &quit])?;

            // Load icon from bundled icons/
            let icon = Image::from_path("icons/icon.ico").unwrap_or_else(|_| app.default_window_icon().unwrap().clone());

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .tooltip("YouTube Desktop")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "pin" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let current = w.is_always_on_top().unwrap_or(false);
                                let _ = w.set_always_on_top(!current);
                            }
                        }
                        "play_pause" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.eval(
                                    "(function(){var b=document.querySelector('.ytp-play-button'); if(b) b.click();})();"
                                );
                            }
                        }
                        "next" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.eval(
                                    "(function(){var b=document.querySelector('.ytp-next-button'); if(b) b.click();})();"
                                );
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;

            // Register global shortcut for play/pause — works even when window is hidden/background
            // ctrl+shift+space because ctrl+space was taken by literally everything else on the planet
            let window_clone = window.clone();
            app.global_shortcut()
                .on_shortcut("ctrl+shift+space", move |_app, _shortcut, _event| {
                    // find the play button and click it like a ghost haunting youtube
                    let _ = window_clone.eval(
                        "(function(){var b=document.querySelector('.ytp-play-button'); if(b) b.click();})()"
                    );
                })
                // if this fails we just silently have no shortcut — not ideal but also not worth crashing over
                .expect("Failed to register global shortcut");


            // Minimize to tray behavior: close/minimize the window instead of quitting the app
            // intercept window events so the X button doesn't actually kill us
            let win_clone = window.clone();
            window.on_window_event(move |event| {
                match event {
                    // user clicked X — nope, we're just hiding. you're not getting rid of us that easily
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                    // user minimized — also hide, because a minimized webview is just a waste of RAM
                    tauri::WindowEvent::Resized(_) => {
                        if let Ok(true) = win_clone.is_minimized() {
                            let _ = win_clone.hide();
                        }
                    }
                    // anything else? not our problem
                    _ => {}
                }
            });

            Ok(())
        })
        // generate_context! reads tauri.conf.json at compile time — don't ask me how, it's macro magic
        .run(tauri::generate_context!())
        // if this panics, something has gone catastrophically wrong and i'm sorry
        .expect("error while running tauri application");
}