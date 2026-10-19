//! Browser application state: the tab list, the address bar and the glue between
//! the Slint UI, the Servo engine and the persistent stores of `nitium-core`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use nitium_core::bookmarks::Bookmarks;
use nitium_core::find::{self, FindCommand};
use nitium_core::history::History;
use nitium_core::ime::{ImeAction, ImeState};
use nitium_core::input::{self, BLANK_URL};
use nitium_core::profile::Profile;
use nitium_core::session::Session;
use nitium_core::settings::Settings;
use nitium_core::unix_now;
use nitium_core::zoom;
use nitium_webview::{
    BrowserEvent, CompositionPhase, ControlId, DeviceRect, DialogKind, PageDialog, PageMenu,
    PageSelect, ServoBrowser, TabId,
};
use slint::platform::Key;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::shortcuts::{self, Modifiers, Shortcut};
use crate::{AppWindow, BrowserLogic, MenuRow, SuggestionInfo, TabInfo, WebviewLogic};

const MAX_CLOSED_TABS: usize = 25;
const MAX_SUGGESTIONS: usize = 8;
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);
/// How long to wait after an input-method preedit is cleared for a commit to follow.
const IME_SETTLE_DELAY: Duration = Duration::from_millis(40);

/// The popup menu currently shown, with what is needed to answer it.
enum Popup {
    Menu(PageMenu),
    Select {
        select: PageSelect,
        /// Identifiers of the options chosen so far (a multi-select is applied on close).
        chosen: Vec<usize>,
    },
}

impl Popup {
    fn control_id(&self) -> ControlId {
        match self {
            Popup::Menu(menu) => menu.control_id(),
            Popup::Select { select, .. } => select.control_id(),
        }
    }
}

pub struct BrowserApp {
    weak_self: Weak<BrowserApp>,
    app: slint::Weak<AppWindow>,
    engine: RefCell<Option<ServoBrowser>>,
    /// Source of truth for the tab strip, in display order.
    tabs: Rc<VecModel<TabInfo>>,
    /// URLs of recently closed tabs, most recent last (for Ctrl+Shift+T).
    closed_urls: RefCell<Vec<String>>,
    /// Pages to open once the engine is up, and the index of the one to show.
    start_urls: Vec<String>,
    start_active: usize,
    dark_mode: Cell<bool>,

    /// `None` when no profile directory could be determined; nothing is persisted then.
    profile: Option<Profile>,
    settings: Settings,
    bookmarks: RefCell<Bookmarks>,
    history: RefCell<History>,
    history_dirty: Cell<bool>,
    autosave: slint::Timer,
    ime: RefCell<ImeState>,
    /// Questions from pages waiting for the user, oldest first.
    dialogs: RefCell<Vec<(TabId, PageDialog)>>,
    popup: RefCell<Option<Popup>>,
    menu_rows: Rc<VecModel<MenuRow>>,
    /// Where the pointer last was over the page, in physical pixels (a right click opens
    /// its context menu there).
    last_pointer: Cell<(f32, f32)>,
}

/// Decides what to open at startup: command-line pages, else the previous session,
/// else the home page. Returns the pages and the index of the active one.
fn startup_pages(
    cli_args: &[String],
    profile: Option<&Profile>,
    settings: &Settings,
) -> (Vec<String>, usize) {
    if !cli_args.is_empty() {
        let urls = cli_args
            .iter()
            .map(|arg| input::resolve(arg, &settings.search_url))
            .collect();
        return (urls, 0);
    }
    if settings.restore_session {
        if let Some(profile) = profile {
            let session = Session::load(profile);
            if !session.is_empty() {
                return (session.tabs, session.active);
            }
        }
    }
    let home = input::resolve(&settings.home_url, &settings.search_url);
    (vec![home], 0)
}

impl BrowserApp {
    /// Wires every UI callback to the browser. The engine itself is created later,
    /// from [`Self::on_rendering_notifier`], once the GPU device exists.
    pub fn install(app: &AppWindow, cli_args: Vec<String>) -> Rc<Self> {
        let profile = Profile::default_location();
        let settings = profile.as_ref().map(Settings::load).unwrap_or_default();
        let bookmarks = profile.as_ref().map(Bookmarks::load).unwrap_or_default();
        let history = profile.as_ref().map(History::load).unwrap_or_default();
        let (start_urls, start_active) = startup_pages(&cli_args, profile.as_ref(), &settings);

        let this = Rc::new_cyclic(|weak| Self {
            weak_self: weak.clone(),
            app: app.as_weak(),
            engine: RefCell::new(None),
            tabs: Rc::new(VecModel::default()),
            closed_urls: RefCell::new(Vec::new()),
            start_urls,
            start_active,
            dark_mode: Cell::new(false),
            profile,
            settings,
            bookmarks: RefCell::new(bookmarks),
            history: RefCell::new(history),
            history_dirty: Cell::new(false),
            autosave: slint::Timer::default(),
            ime: RefCell::new(ImeState::default()),
            dialogs: RefCell::new(Vec::new()),
            popup: RefCell::new(None),
            menu_rows: Rc::new(VecModel::default()),
            last_pointer: Cell::new((0.0, 0.0)),
        });

        {
            let weak = Rc::downgrade(&this);
            this.autosave
                .start(slint::TimerMode::Repeated, AUTOSAVE_INTERVAL, move || {
                    if let Some(this) = weak.upgrade() {
                        this.save_all();
                    }
                });
        }

        let browser = app.global::<BrowserLogic>();
        browser.set_tabs(ModelRc::from(this.tabs.clone()));
        browser.set_menu_rows(ModelRc::from(this.menu_rows.clone()));
        {
            let this = this.clone();
            browser.on_new_tab(move || this.new_tab());
        }
        {
            let this = this.clone();
            browser.on_close_tab(move |id| this.close_tab(id as TabId));
        }
        {
            let this = this.clone();
            browser.on_select_tab(move |id| this.select_tab(id as TabId));
        }
        {
            let this = this.clone();
            browser.on_navigate(move |text| this.navigate(text.as_str()));
        }
        {
            let this = this.clone();
            browser.on_address_edited(move |text| this.address_edited(text.as_str()));
        }
        {
            let this = this.clone();
            browser.on_toggle_bookmark(move || this.toggle_bookmark());
        }
        {
            let this = this.clone();
            browser.on_back(move || this.with_engine(|engine| engine.go_back()));
        }
        {
            let this = this.clone();
            browser.on_forward(move || this.with_engine(|engine| engine.go_forward()));
        }
        {
            let this = this.clone();
            browser.on_reload(move || this.with_engine(|engine| engine.reload()));
        }
        {
            let this = this.clone();
            browser.on_shortcut(move |event| this.handle_shortcut(&event));
        }
        {
            let this = this.clone();
            browser.on_dialog_accept(move || this.answer_dialog(true));
        }
        {
            let this = this.clone();
            browser.on_dialog_cancel(move || this.answer_dialog(false));
        }
        {
            let this = this.clone();
            browser.on_menu_activate(move |index| this.menu_activate(index.max(0) as usize));
        }
        {
            let this = this.clone();
            browser.on_menu_dismiss(move || this.menu_dismiss());
        }
        {
            let this = this.clone();
            browser.on_find_changed(move |text| this.find_search(text.as_str()));
        }
        {
            let this = this.clone();
            browser.on_find_next(move || this.find_step(true));
        }
        {
            let this = this.clone();
            browser.on_find_previous(move || this.find_step(false));
        }
        {
            let this = this.clone();
            browser.on_find_close(move || this.close_find());
        }
        {
            let this = this.clone();
            browser.on_reset_zoom(move || this.zoom_by(|_| zoom::DEFAULT));
        }

        let webview = app.global::<WebviewLogic>();
        {
            let this = this.clone();
            webview.on_theme(move |scheme| {
                let dark = scheme == slint::language::ColorScheme::Dark;
                this.dark_mode.set(dark);
                this.with_engine(|engine| engine.set_theme(dark));
            });
        }
        {
            let this = this.clone();
            // The size arrives in physical pixels (`physical-length` in the .slint).
            webview.on_resize(move |width, height| {
                let scale = this.scale_factor();
                this.with_engine(|engine| engine.resize(width, height, scale));
            });
        }
        {
            let this = this.clone();
            webview.on_scroll(move |x, y, dx, dy| {
                this.with_engine(|engine| engine.scroll(x, y, dx, dy));
            });
        }
        {
            let this = this.clone();
            webview.on_pointer(move |event, x, y| {
                this.last_pointer.set((x, y));
                this.with_engine(|engine| engine.handle_pointer(&event, x, y));
            });
        }
        {
            let this = this.clone();
            webview.on_key_event(move |event, is_pressed| {
                this.with_engine(|engine| engine.handle_key_event(&event, is_pressed));
            });
        }
        {
            let this = this.clone();
            webview.on_ime_preedit(move |text| this.ime_preedit(text.as_str()));
        }
        {
            let this = this.clone();
            webview.on_ime_commit(move |text| this.ime_commit(text.as_str()));
        }

        this
    }

    /// Creates the engine on the first `RenderingSetup`, then opens the start pages.
    pub fn on_rendering_notifier(
        self: &Rc<Self>,
        state: &slint::RenderingState,
        graphics_api: &slint::GraphicsAPI,
    ) {
        if !matches!(state, slint::RenderingState::RenderingSetup) || self.engine.borrow().is_some()
        {
            return;
        }
        let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api else {
            eprintln!("nitium: GraphicsAPI is not WGPU30, got: {graphics_api:?}");
            return;
        };
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<WebviewLogic>();
        let width = logic.get_viewport_width().max(800.0);
        let height = logic.get_viewport_height().max(600.0);
        let scale = app.window().scale_factor();

        eprintln!("nitium: Initializing ServoBrowser ({width}x{height} @ scale {scale})...");
        let weak = Rc::downgrade(self);
        let engine = ServoBrowser::new(
            device.clone(),
            queue.clone(),
            width,
            height,
            scale,
            self.dark_mode.get(),
            move |event| {
                if let Some(this) = weak.upgrade() {
                    this.handle_event(event);
                }
            },
        );
        match engine {
            Ok(engine) => *self.engine.borrow_mut() = Some(engine),
            Err(err) => {
                eprintln!("nitium: Failed to initialize Servo browser: {err}");
                return;
            }
        }

        for (index, url) in self.start_urls.iter().enumerate() {
            self.open_tab(url, index == self.start_active);
        }
    }

    /// Writes history and the open-tabs session to the profile. Called periodically and
    /// on exit; bookmarks are saved the moment they change.
    pub fn save_all(&self) {
        let Some(profile) = &self.profile else {
            return;
        };
        if self.history_dirty.replace(false) {
            if let Err(err) = self.history.borrow().save(profile) {
                eprintln!("nitium: cannot save history: {err}");
                self.history_dirty.set(true);
            }
        }
        // Without an engine no tab was ever opened; saving would wipe the last session.
        if self.engine.borrow().is_some() {
            let urls: Vec<String> = self
                .tabs
                .iter()
                .map(|row| row.url.to_string())
                .collect();
            let session = Session::from_tabs(&urls, self.active_index().unwrap_or(0));
            if let Err(err) = session.save(profile) {
                eprintln!("nitium: cannot save session: {err}");
            }
        }
    }

    // -- Engine events ------------------------------------------------------

    fn handle_event(&self, event: BrowserEvent) {
        match event {
            BrowserEvent::Frame(image) => {
                if let Some(app) = self.app.upgrade() {
                    app.global::<WebviewLogic>().set_web_content(image);
                    app.window().request_redraw();
                }
            }
            BrowserEvent::TabOpened { tab } => {
                // Opened by a page: its URL is not known yet, but it is not for typing into.
                self.tabs.push(new_row(tab, BLANK_URL));
                self.show_tab(tab, false);
            }
            BrowserEvent::TabClosed { tab } => self.close_tab(tab),
            BrowserEvent::Url { tab, url } => {
                // A different page has no use for the previous page's search results.
                let navigated = self
                    .row_index(tab)
                    .and_then(|i| self.tabs.row_data(i))
                    .is_some_and(|row| {
                        row.active
                            && input::without_fragment(row.url.as_str())
                                != input::without_fragment(&url)
                    });
                if navigated {
                    self.close_find();
                }
                self.history.borrow_mut().record_visit(&url, unix_now());
                self.history_dirty.set(true);
                self.update_row(tab, |row| row.url = url.into());
            }
            BrowserEvent::Title { tab, title } => {
                let title = title.unwrap_or_default();
                if let Some(row) = self.row_index(tab).and_then(|i| self.tabs.row_data(i)) {
                    self.history.borrow_mut().set_title(row.url.as_str(), &title);
                    self.history_dirty.set(true);
                }
                self.update_row(tab, |row| row.title = title.into());
            }
            BrowserEvent::InputMethod { tab, field } => {
                if self.is_active(tab) {
                    self.set_ime_field(field);
                }
            }
            BrowserEvent::Cursor { tab, cursor } => {
                if let (true, Some(app)) = (self.is_active(tab), self.app.upgrade()) {
                    app.global::<WebviewLogic>().set_cursor(cursor.into());
                }
            }
            BrowserEvent::Dialog { tab, dialog } => self.queue_dialog(tab, dialog),
            BrowserEvent::ContextMenu { tab, menu } => self.open_menu(tab, menu),
            BrowserEvent::SelectPopup { tab, select } => self.open_select(tab, select),
            BrowserEvent::ControlHidden { id, .. } => self.control_hidden(id),
            BrowserEvent::Loading { tab, loading } => {
                self.update_row(tab, |row| row.loading = loading);
            }
            BrowserEvent::History {
                tab,
                can_go_back,
                can_go_forward,
            } => {
                self.update_row(tab, |row| {
                    row.can_go_back = can_go_back;
                    row.can_go_forward = can_go_forward;
                });
            }
        }
    }

    // -- Tabs ---------------------------------------------------------------

    fn new_tab(&self) {
        self.open_tab(BLANK_URL, true);
    }

    /// Opens `url` (already resolved to an absolute URL) in a new tab.
    fn open_tab(&self, url: &str, activate: bool) {
        let Some(tab) = self.engine.borrow().as_ref().map(|e| e.open_tab(url, false)) else {
            return;
        };
        self.tabs.push(new_row(tab, url));
        if activate || self.tabs.row_count() == 1 {
            self.select_tab(tab);
        }
    }

    fn select_tab(&self, tab: TabId) {
        self.show_tab(tab, true);
    }

    /// Brings `tab` to the front. A blank tab gets the address bar's focus only when
    /// `address_if_blank` (tabs the user opened); any other tab focuses the page.
    fn show_tab(&self, tab: TabId, address_if_blank: bool) {
        let Some(index) = self.row_index(tab) else {
            return;
        };
        // Per-page state of the tab being left must not leak into the next one.
        self.close_find();
        self.dismiss_popup();
        self.set_ime_field(None);
        if let Some(app) = self.app.upgrade() {
            app.global::<WebviewLogic>().set_cursor("default".into());
        }
        self.with_engine(|engine| engine.activate_tab(tab));
        for i in 0..self.tabs.row_count() {
            let mut row = self.tabs.row_data(i).expect("row in range");
            let active = i == index;
            if row.active != active {
                row.active = active;
                self.tabs.set_row_data(i, row);
            }
        }
        self.clear_suggestions();
        self.sync_active(true);
        self.refresh_dialog();

        if let Some(app) = self.app.upgrade() {
            let row = self.tabs.row_data(index).expect("row in range");
            if address_if_blank && input::display_url(row.url.as_str()).is_empty() {
                app.invoke_focus_address();
            } else {
                app.invoke_focus_content();
            }
        }
    }

    fn close_tab(&self, tab: TabId) {
        let Some(index) = self.row_index(tab) else {
            return;
        };
        let row = self.tabs.row_data(index).expect("row in range");
        if input::is_web_url(row.url.as_str()) {
            let mut closed = self.closed_urls.borrow_mut();
            closed.push(row.url.to_string());
            if closed.len() > MAX_CLOSED_TABS {
                closed.remove(0);
            }
        }

        // Questions from a closed page can no longer be answered; dropping them dismisses them.
        self.dialogs.borrow_mut().retain(|(owner, _)| *owner != tab);
        self.with_engine(|engine| engine.close_tab(tab));
        self.tabs.remove(index);

        if self.tabs.row_count() == 0 {
            let _ = slint::quit_event_loop();
            return;
        }
        if row.active {
            let next = index.min(self.tabs.row_count() - 1);
            let next_id = self.tabs.row_data(next).expect("row in range").id;
            self.select_tab(next_id as TabId);
        }
    }

    fn select_relative(&self, delta: isize) {
        let count = self.tabs.row_count() as isize;
        let Some(active) = self.active_index() else {
            return;
        };
        let next = (active as isize + delta).rem_euclid(count) as usize;
        let id = self.tabs.row_data(next).expect("row in range").id;
        self.select_tab(id as TabId);
    }

    fn select_index(&self, index: usize) {
        if let Some(row) = self.tabs.row_data(index) {
            self.select_tab(row.id as TabId);
        }
    }

    fn reopen_closed_tab(&self) {
        let url = self.closed_urls.borrow_mut().pop();
        if let Some(url) = url {
            self.open_tab(&url, true);
        }
    }

    // -- Navigation ---------------------------------------------------------

    /// Loads whatever the user typed (URL, domain or search terms) in the active tab.
    fn navigate(&self, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let url = input::resolve(text, &self.settings.search_url);
        self.with_engine(|engine| engine.load_url(&url));
        self.clear_suggestions();
        if let Some(app) = self.app.upgrade() {
            app.invoke_focus_content();
        }
    }

    // -- Bookmarks and suggestions -------------------------------------------

    fn toggle_bookmark(&self) {
        let Some(row) = self.active_index().and_then(|i| self.tabs.row_data(i)) else {
            return;
        };
        self.bookmarks
            .borrow_mut()
            .toggle(row.url.as_str(), row.title.as_str(), unix_now());
        if let Some(profile) = &self.profile {
            if let Err(err) = self.bookmarks.borrow().save(profile) {
                eprintln!("nitium: cannot save bookmarks: {err}");
            }
        }
        self.sync_active(false);
    }

    /// Bookmarks first, then history, without duplicates.
    fn suggestions_for(&self, text: &str) -> Vec<SuggestionInfo> {
        let mut out: Vec<SuggestionInfo> = Vec::new();
        let mut push = |title: &str, url: &str| {
            if out.len() >= MAX_SUGGESTIONS || out.iter().any(|s| s.url.as_str() == url) {
                return;
            }
            let title = if title.is_empty() { url } else { title };
            out.push(SuggestionInfo {
                title: title.into(),
                url: url.into(),
            });
        };
        for bookmark in self.bookmarks.borrow().search(text, MAX_SUGGESTIONS) {
            push(&bookmark.title, &bookmark.url);
        }
        for entry in self.history.borrow().suggest(text, MAX_SUGGESTIONS) {
            push(&entry.title, &entry.url);
        }
        out
    }

    fn address_edited(&self, text: &str) {
        self.set_suggestions(self.suggestions_for(text));
    }

    fn clear_suggestions(&self) {
        self.set_suggestions(Vec::new());
    }

    fn set_suggestions(&self, suggestions: Vec<SuggestionInfo>) {
        if let Some(app) = self.app.upgrade() {
            let logic = app.global::<BrowserLogic>();
            logic.set_suggestions(ModelRc::new(VecModel::from(suggestions)));
            logic.set_suggestion_index(-1);
        }
    }

    // -- Shortcuts ----------------------------------------------------------

    /// Returns true when the key was consumed by the application.
    fn handle_shortcut(&self, event: &slint::language::KeyEvent) -> bool {
        // Escape in the address bar abandons the edit and returns to the page.
        if event.text == SharedString::from(Key::Escape) {
            let Some(app) = self.app.upgrade() else {
                return false;
            };
            let logic = app.global::<BrowserLogic>();
            if logic.get_menu_visible() {
                self.menu_dismiss();
                return true;
            }
            if logic.get_address_focused() {
                self.clear_suggestions();
                self.sync_active(true);
                app.invoke_focus_content();
                return true;
            }
            if logic.get_find_focused() {
                self.close_find();
                return true;
            }
            return false;
        }

        let mods = Modifiers {
            control: event.modifiers.control,
            shift: event.modifiers.shift,
            alt: event.modifiers.alt,
            meta: event.modifiers.meta,
        };
        let Some(shortcut) = shortcuts::resolve(event.text.as_str(), mods) else {
            return false;
        };
        match shortcut {
            Shortcut::NewTab => self.new_tab(),
            Shortcut::CloseTab => {
                if let Some(index) = self.active_index() {
                    let id = self.tabs.row_data(index).expect("row in range").id;
                    self.close_tab(id as TabId);
                }
            }
            Shortcut::ReopenClosedTab => self.reopen_closed_tab(),
            Shortcut::FocusAddress => {
                if let Some(app) = self.app.upgrade() {
                    app.invoke_focus_address();
                }
            }
            Shortcut::ToggleBookmark => self.toggle_bookmark(),
            Shortcut::ZoomIn => self.zoom_by(zoom::zoom_in),
            Shortcut::ZoomOut => self.zoom_by(zoom::zoom_out),
            Shortcut::ZoomReset => self.zoom_by(|_| zoom::DEFAULT),
            Shortcut::Find => self.open_find(),
            Shortcut::FindNext => self.find_step(true),
            Shortcut::FindPrevious => self.find_step(false),
            Shortcut::Reload => self.with_engine(|engine| engine.reload()),
            Shortcut::Back => self.with_engine(|engine| engine.go_back()),
            Shortcut::Forward => self.with_engine(|engine| engine.go_forward()),
            Shortcut::NextTab => self.select_relative(1),
            Shortcut::PreviousTab => self.select_relative(-1),
            Shortcut::SelectTab(index) => self.select_index(index),
            Shortcut::SelectLastTab => {
                if let Some(last) = self.tabs.row_count().checked_sub(1) {
                    self.select_index(last);
                }
            }
        }
        true
    }

    // -- Input method -----------------------------------------------------------

    /// A text field in the page gained (`Some`) or lost (`None`) focus.
    fn set_ime_field(&self, field: Option<DeviceRect>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<WebviewLogic>();
        match field {
            Some(area) => {
                // The engine reports device pixels; the UI works in logical ones.
                let scale = app.window().scale_factor();
                logic.set_ime_x(area.x / scale);
                logic.set_ime_y(area.y / scale);
                logic.set_ime_width(area.width / scale);
                logic.set_ime_height(area.height / scale);
                logic.set_ime_active(true);
            }
            None => {
                logic.set_ime_active(false);
                if self.ime.borrow_mut().reset() {
                    self.with_engine(|engine| engine.ime_dismiss());
                }
            }
        }
    }

    fn ime_preedit(&self, text: &str) {
        let actions = self.ime.borrow_mut().preedit(text);
        self.apply_ime(actions);
        if text.is_empty() {
            // Toolkits disagree on whether "preedit cleared" comes before or after the
            // commit, so only treat it as a cancellation if no commit follows shortly.
            let weak = self.weak_self.clone();
            slint::Timer::single_shot(IME_SETTLE_DELAY, move || {
                if let Some(this) = weak.upgrade() {
                    let actions = this.ime.borrow_mut().settle();
                    this.apply_ime(actions);
                }
            });
        }
    }

    fn ime_commit(&self, text: &str) {
        let actions = self.ime.borrow_mut().commit(text);
        self.apply_ime(actions);
    }

    fn apply_ime(&self, actions: Vec<ImeAction>) {
        if actions.is_empty() {
            return;
        }
        self.with_engine(|engine| {
            for action in actions {
                match action {
                    ImeAction::Start => engine.ime_composition(CompositionPhase::Start, ""),
                    ImeAction::Update(text) => {
                        engine.ime_composition(CompositionPhase::Update, &text)
                    }
                    ImeAction::End(text) => engine.ime_composition(CompositionPhase::End, &text),
                }
            }
        });
    }

    // -- Zoom ---------------------------------------------------------------------

    /// Sets the active page's zoom to `next(current)`.
    fn zoom_by(&self, next: impl FnOnce(f32) -> f32) {
        let current = self.engine.borrow().as_ref().map(|engine| engine.zoom());
        let Some(current) = current else {
            return;
        };
        self.with_engine(|engine| engine.set_zoom(next(current)));
        self.sync_active(false);
    }

    // -- Find in page ---------------------------------------------------------------

    fn open_find(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        logic.set_find_open(true);
        // The bar only becomes visible, hence focusable, once this change has been applied.
        let window = self.app.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(app) = window.upgrade() {
                app.invoke_focus_find();
            }
        });
        let query = logic.get_find_text();
        if !query.is_empty() {
            self.find_search(query.as_str());
        }
    }

    fn close_find(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        if !logic.get_find_open() {
            return;
        }
        logic.set_find_open(false);
        logic.set_find_status(SharedString::new());
        self.run_find(FindCommand::Clear);
        app.invoke_focus_content();
    }

    fn find_search(&self, query: &str) {
        if query.is_empty() {
            if let Some(app) = self.app.upgrade() {
                app.global::<BrowserLogic>()
                    .set_find_status(SharedString::new());
            }
            self.run_find(FindCommand::Clear);
            return;
        }
        // Smart case: a query with a capital letter is matched case-sensitively.
        let case_sensitive = query.chars().any(char::is_uppercase);
        self.run_find(FindCommand::Search {
            query,
            case_sensitive,
        });
    }

    /// Next/previous match; opens the bar first if it is closed.
    fn find_step(&self, forward: bool) {
        let open = self
            .app
            .upgrade()
            .is_some_and(|app| app.global::<BrowserLogic>().get_find_open());
        if !open {
            self.open_find();
            return;
        }
        self.run_find(if forward {
            FindCommand::Next
        } else {
            FindCommand::Previous
        });
    }

    fn run_find(&self, command: FindCommand<'_>) {
        let script = find::script(command);
        let wants_status = !matches!(command, FindCommand::Clear);
        let weak = self.weak_self.clone();
        self.with_engine(|engine| {
            engine.evaluate_script(&script, move |answer| {
                if !wants_status {
                    return;
                }
                if let Some(this) = weak.upgrade() {
                    this.show_find_status(answer);
                }
            });
        });
    }

    fn show_find_status(&self, answer: Option<String>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        let text = match answer {
            // The script could not run (page still loading, …): show nothing.
            None => String::new(),
            _ if !logic.get_find_open() || logic.get_find_text().is_empty() => String::new(),
            Some(answer) => {
                let status = find::parse_status(&answer);
                if status.total == 0 {
                    "No matches".to_string()
                } else {
                    format!("{}/{}", status.current, status.total)
                }
            }
        };
        logic.set_find_status(text.into());
    }

    // -- Questions from the page ------------------------------------------------------

    fn active_tab_id(&self) -> Option<TabId> {
        self.active_index()
            .and_then(|i| self.tabs.row_data(i))
            .map(|row| row.id as TabId)
    }

    fn queue_dialog(&self, tab: TabId, dialog: PageDialog) {
        self.dialogs.borrow_mut().push((tab, dialog));
        let showing = self
            .app
            .upgrade()
            .is_some_and(|app| app.global::<BrowserLogic>().get_dialog_visible());
        if !showing {
            self.refresh_dialog();
        }
    }

    /// Shows the oldest question asked by the active tab, or hides the dialog if there is none.
    fn refresh_dialog(&self) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        let active = self.active_tab_id();
        // (kind, title, message, initial text)
        let front = self
            .dialogs
            .borrow()
            .iter()
            .find(|(tab, _)| Some(*tab) == active)
            .map(|(_, dialog)| {
                let (kind, title) = match dialog.kind() {
                    DialogKind::Alert => (0, format!("{} says", dialog.origin())),
                    DialogKind::Confirm => (1, format!("{} says", dialog.origin())),
                    DialogKind::Prompt => (2, format!("{} says", dialog.origin())),
                    DialogKind::Permission => (3, "Permission request".to_string()),
                    DialogKind::Authentication => (4, "Sign in".to_string()),
                };
                (
                    kind,
                    title,
                    dialog.message().to_string(),
                    dialog.default_text().to_string(),
                )
            });
        let Some((kind, title, message, initial)) = front else {
            logic.set_dialog_visible(false);
            return;
        };
        logic.set_dialog_kind(kind);
        logic.set_dialog_title(title.into());
        logic.set_dialog_message(message.into());
        logic.set_dialog_input(initial.into());
        logic.set_dialog_password(SharedString::new());
        logic.set_dialog_visible(true);
        // The dialog only becomes visible, hence focusable, once this change has been applied.
        let window = self.app.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(app) = window.upgrade() {
                app.invoke_focus_dialog();
            }
        });
    }

    /// OK / Allow / Sign in (`accept`) or Cancel / Deny for the dialog on screen.
    fn answer_dialog(&self, accept: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        let active = self.active_tab_id();
        let dialog = {
            let mut dialogs = self.dialogs.borrow_mut();
            let position = dialogs.iter().position(|(tab, _)| Some(*tab) == active);
            position.map(|position| dialogs.remove(position).1)
        };
        let Some(dialog) = dialog else {
            logic.set_dialog_visible(false);
            return;
        };
        let input = logic.get_dialog_input();
        let password = logic.get_dialog_password();
        // Answered with no borrow held: the engine may react by raising new events.
        if accept {
            dialog.accept(input.as_str(), password.as_str());
        } else {
            dialog.cancel();
        }
        logic.set_dialog_password(SharedString::new());
        self.refresh_dialog();
        if !logic.get_dialog_visible() {
            app.invoke_focus_content();
        }
    }

    /// The engine withdrew a control: forget it if it is still on screen.
    fn control_hidden(&self, id: ControlId) {
        let removed = {
            let mut dialogs = self.dialogs.borrow_mut();
            let before = dialogs.len();
            dialogs.retain(|(_, dialog)| dialog.control_id() != Some(id));
            dialogs.len() != before
        };
        if removed {
            self.refresh_dialog();
        }
        let popup_withdrawn = self
            .popup
            .borrow()
            .as_ref()
            .is_some_and(|popup| popup.control_id() == id);
        if popup_withdrawn {
            self.dismiss_popup();
        }
    }

    // -- Popup menus (context menu, <select>) -------------------------------------------

    fn open_menu(&self, tab: TabId, menu: PageMenu) {
        if !self.is_active(tab) {
            return;
        }
        let rows: Vec<MenuRow> = menu
            .entries()
            .iter()
            .map(|entry| MenuRow {
                label: entry.label.as_str().into(),
                enabled: entry.enabled,
                separator: entry.separator,
                checked: false,
                heading: false,
                indented: false,
            })
            .collect();
        // A context menu opens where the user clicked.
        let (x, y) = self.last_pointer.get();
        let scale = self.scale_factor();
        self.show_popup(rows, x / scale, y / scale, 0.0, Popup::Menu(menu));
    }

    fn open_select(&self, tab: TabId, select: PageSelect) {
        if !self.is_active(tab) {
            return;
        }
        let rows: Vec<MenuRow> = select
            .choices()
            .iter()
            .map(|choice| MenuRow {
                label: choice.label.as_str().into(),
                enabled: choice.selectable,
                separator: false,
                checked: choice.selected,
                heading: choice.heading,
                indented: choice.indented,
            })
            .collect();
        let chosen = select
            .choices()
            .iter()
            .filter(|choice| choice.selected && !choice.heading)
            .map(|choice| choice.id)
            .collect();
        // A list drops down from the bottom edge of its element, as wide as the element.
        let area = select.area();
        let scale = self.scale_factor();
        self.show_popup(
            rows,
            area.x / scale,
            (area.y + area.height) / scale,
            area.width / scale,
            Popup::Select { select, chosen },
        );
    }

    fn show_popup(&self, rows: Vec<MenuRow>, x: f32, y: f32, min_width: f32, popup: Popup) {
        self.dismiss_popup();
        let Some(app) = self.app.upgrade() else {
            return;
        };
        // Rows are 28px (9px for separators) plus 4px padding above and below.
        let height: f32 = 8.0
            + rows
                .iter()
                .map(|row| if row.separator { 9.0 } else { 28.0 })
                .sum::<f32>();
        self.menu_rows.set_vec(rows);
        let logic = app.global::<BrowserLogic>();
        logic.set_menu_x(x);
        logic.set_menu_y(y);
        logic.set_menu_width(min_width);
        logic.set_menu_height(height);
        logic.set_menu_visible(true);
        *self.popup.borrow_mut() = Some(popup);
    }

    fn menu_activate(&self, index: usize) {
        // Taken out of the cell first: answering may make the engine raise new events.
        let popup = self.popup.borrow_mut().take();
        match popup {
            None => {}
            Some(Popup::Menu(menu)) => {
                self.hide_popup();
                menu.select(index);
                self.focus_page();
            }
            Some(Popup::Select { select, mut chosen }) => {
                let choice = select.choices().get(index).cloned();
                match choice {
                    Some(choice) if choice.selectable && !choice.heading => {
                        if select.multiple() {
                            // Stay open: toggle this option and wait for the user to finish.
                            match chosen.iter().position(|&id| id == choice.id) {
                                Some(position) => {
                                    chosen.remove(position);
                                }
                                None => chosen.push(choice.id),
                            }
                            if let Some(mut row) = self.menu_rows.row_data(index) {
                                row.checked = chosen.contains(&choice.id);
                                self.menu_rows.set_row_data(index, row);
                            }
                            *self.popup.borrow_mut() = Some(Popup::Select { select, chosen });
                        } else {
                            self.hide_popup();
                            select.submit(vec![choice.id]);
                            self.focus_page();
                        }
                    }
                    // A heading or a disabled option: nothing happens, the list stays.
                    _ => *self.popup.borrow_mut() = Some(Popup::Select { select, chosen }),
                }
            }
        }
    }

    /// A click outside the menu: closes it. A multi-select applies what was ticked.
    fn menu_dismiss(&self) {
        let popup = self.popup.borrow_mut().take();
        self.hide_popup();
        if let Some(Popup::Select { select, chosen }) = popup {
            if select.multiple() {
                select.submit(chosen);
            }
        }
        self.focus_page();
    }

    /// Closes the popup without answering it (the engine treats that as "no change").
    fn dismiss_popup(&self) {
        let popup = self.popup.borrow_mut().take();
        if popup.is_some() {
            self.hide_popup();
        }
    }

    fn hide_popup(&self) {
        if let Some(app) = self.app.upgrade() {
            app.global::<BrowserLogic>().set_menu_visible(false);
        }
    }

    fn focus_page(&self) {
        if let Some(app) = self.app.upgrade() {
            app.invoke_focus_content();
        }
    }

    // -- Helpers ------------------------------------------------------------

    fn is_active(&self, tab: TabId) -> bool {
        self.row_index(tab)
            .and_then(|i| self.tabs.row_data(i))
            .is_some_and(|row| row.active)
    }

    fn with_engine(&self, f: impl FnOnce(&ServoBrowser)) {
        if let Some(engine) = self.engine.borrow().as_ref() {
            f(engine);
        }
    }

    fn scale_factor(&self) -> f32 {
        self.app
            .upgrade()
            .map_or(1.0, |app| app.window().scale_factor())
    }

    fn row_index(&self, tab: TabId) -> Option<usize> {
        (0..self.tabs.row_count()).find(|&i| {
            self.tabs
                .row_data(i)
                .is_some_and(|row| row.id == tab as i32)
        })
    }

    fn active_index(&self) -> Option<usize> {
        (0..self.tabs.row_count()).find(|&i| self.tabs.row_data(i).is_some_and(|row| row.active))
    }

    /// Edits a tab's row and refreshes the toolbar if it is the active tab.
    fn update_row(&self, tab: TabId, edit: impl FnOnce(&mut TabInfo)) {
        let Some(index) = self.row_index(tab) else {
            return;
        };
        let mut row = self.tabs.row_data(index).expect("row in range");
        edit(&mut row);
        let active = row.active;
        self.tabs.set_row_data(index, row);
        if active {
            self.sync_active(false);
        }
    }

    /// Mirrors the active tab into the toolbar, address bar and window title.
    /// The address is left alone while the user is typing unless `force_address`.
    fn sync_active(&self, force_address: bool) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        let logic = app.global::<BrowserLogic>();
        let Some(row) = self.active_index().and_then(|i| self.tabs.row_data(i)) else {
            return;
        };
        logic.set_can_go_back(row.can_go_back);
        logic.set_can_go_forward(row.can_go_forward);
        logic.set_loading(row.loading);
        let page_zoom = self
            .engine
            .borrow()
            .as_ref()
            .map_or(zoom::DEFAULT, |engine| engine.zoom());
        logic.set_zoom_percent(zoom::percent(page_zoom));
        logic.set_bookmarked(self.bookmarks.borrow().contains(row.url.as_str()));
        logic.set_window_title(if row.title.is_empty() {
            "Nitium".into()
        } else {
            format!("{} — Nitium", row.title).into()
        });
        if force_address || !logic.get_address_focused() {
            logic.set_address(input::display_url(row.url.as_str()).into());
        }
    }
}

fn new_row(tab: TabId, url: &str) -> TabInfo {
    TabInfo {
        id: tab as i32,
        title: SharedString::new(),
        url: url.into(),
        loading: false,
        active: false,
        can_go_back: false,
        can_go_forward: false,
    }
}
