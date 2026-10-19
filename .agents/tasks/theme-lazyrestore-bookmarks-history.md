# Theme Settings, Lazy Session Restore, and Bookmarks & History UI

## 1. Origiri Theme Settings
- [x] Extend `nitium-settings` with theme preset (`TokyoNight`, `Catppuccin Mocha`, `Catppuccin Latte`, `Flexoki Dark`, `Flexoki Light`, `Default Dark`, `Default Light`, `System`), optional accent color hex, and corner radius (`None`, `Small`, `Medium`, `Large`, `Full`)
- [x] Add unit tests in `nitium-settings`
- [x] In `nitium-shell`, apply `origiri_theme::resolve` with `origiri::push_color_tokens!` dynamically onto Slint `Tokens`
- [x] Update `SettingsLayer` in `app.slint` with theme preset selection, accent colors, and corner radius buttons
- [x] Verify fast checks: `just check-fast` and `just test-fast`

## 2. Lazy Session Restore
- [x] In `nitium-core::tabs` or `nitium-shell::browser`, track whether a tab is actively loaded in the web engine
- [x] On startup session restore, instantiate all tabs in the tab strip but only load the active tab in WebView
- [x] When an inactive/lazy tab is selected, trigger engine navigation to its URL
- [x] Unit tests for lazy tab tracking in `nitium-core`
- [x] Update `.agents/tasks/lazy-session-restore.md`

## 3. Bookmarks & History UI
- [x] In `nitium-core::bookmarks`: add methods for item listing, sorting, and remove by id/url
- [x] In `nitium-core::history`: add methods for grouped history by date, remove entry by url, and limit queries
- [x] Add shortcuts: `Ctrl+Shift+O` for Bookmarks, `Ctrl+H` for History
- [x] In `nitium-shell/ui/app.slint`: implement `BookmarksLayer` and `HistoryLayer` overlays
- [x] Wire callbacks in `browser.rs`
- [x] Update `.agents/tasks/bookmarks-history-ui.md`

## 4. Verification & Sync
- [x] `just check-fast`, `just test-fast`
- [x] `just docs-check`
- [x] Full `cargo check -p nitium-desktop`
