// build script — tauri needs this or it throws a tantrum during compilation.
//
// The app_manifest(...) bit below is NOT decoration: Tauri's ACL only trusts commands
// invoked from the local "main" webview (index.html) by default. The tab webviews load
// real https://www.youtube.com content — a remote origin — and Tauri silently DENIES
// any invoke() from remote content unless the command has a generated permission that's
// then allowed in capabilities/default.json. Without this, invoke() from a YouTube page
// fails with "not allowed. Plugin not found" and just silently no-ops. Every custom
// command that a tab webview (not just the titlebar) needs to call must be listed here.
fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "tabs_list",
            "profile_list",
            "profile_select",
            "profile_create",
            "tab_new",
            "tab_new_url",
            "tab_close",
            "tab_switch",
            "tab_navigated",
            "queue_list",
            "queue_add",
            "queue_remove",
            "queue_move",
            "queue_clear",
            "queue_play",
            "queue_play_next",
            "queue_toggle",
            "clean_view_toggle",
            "history_list",
            "bookmark_list",
            "bookmark_add",
            "bookmark_remove",
            "library_panel_toggle",
            "tab_back",
            "tab_forward",
            "tab_reload",
            "tab_navigate",
            "tab_toggle_theater",
            "win_minimize",
            "win_toggle_maximize",
            "win_toggle_fullscreen",
            "win_close",
            "webview_fullscreen_changed",
            "set_titlebar_hidden",
            "show_titlebar",
        ])),
    )
    .expect("failed to run tauri-build");
}
