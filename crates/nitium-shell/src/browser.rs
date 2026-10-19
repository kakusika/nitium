//! Browser application state: the tab list, the address bar and the glue between
//! the Slint UI, the [`WebEngine`] and the persistent stores of `nitium-core`.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use nitium_core::bookmarks::Bookmarks;
use nitium_core::controls::{
    CompositionPhase, ControlId, DeviceRect, DialogKind, PageDialog, PageMenu, PageSelect, TabId,
};
use nitium_core::find::{self, FindCommand};
use nitium_core::history::History;
use nitium_core::ime::{ImeAction, ImeState};
use nitium_core::input::{self, BLANK_URL};
use nitium_core::profile::Profile;
use nitium_core::session::{Session, SessionTab};
use nitium_core::settings::{SEARCH_PRESETS, Settings, SettingsExt, ThemeMode};
use nitium_core::shortcuts::{self, Modifiers, Shortcut};
use nitium_core::tabs::{CloseOutcome, TabEntry, TabList};
use nitium_core::unix_now;
use nitium_core::zoom;
use slint::platform::Key;
use slint::{Global, Model, ModelRc, SharedString, VecModel};

use crate::{
    BookmarkItem, BrowserEvent, BrowserLogic, EngineFactory, HistoryItem, MenuRow,
    SuggestionInfo, TabInfo, Tokens, WebEngine, WebviewLogic,
};

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

/// Abstraction over the host window for scaling and redraw requests.
#[derive(Clone)]
pub struct BrowserHost {
    pub scale_factor: Rc<dyn Fn() -> f32>,
    pub request_redraw: Rc<dyn Fn()>,
}

impl BrowserHost {
    pub fn new<FScale, FRedraw>(scale_factor: FScale, request_redraw: FRedraw) -> Self
    where
        FScale: Fn() -> f32 + 'static,
        FRedraw: Fn() + 'static,
    {
        Self {
            scale_factor: Rc::new(scale_factor),
            request_redraw: Rc::new(request_redraw),
        }
    }

    pub fn scale_factor(&self) -> f32 {
        (self.scale_factor)()
    }

    pub fn request_redraw(&self) {
        (self.request_redraw)();
    }
}

pub struct BrowserApp {
    weak_self: Weak<BrowserApp>,
    host: BrowserHost,
    browser: slint::Weak<BrowserLogic<'static>>,
    webview: slint::Weak<WebviewLogic<'static>>,
    tokens: slint::Weak<Tokens<'static>>,
    engine_factory: EngineFactory,
    engine: RefCell<Option<Box<dyn WebEngine>>>,
    /// Pure domain model for open tabs and the closed-tab stack.
    tab_list: RefCell<TabList>,
    /// Slint view model mirrored from `tab_list`.
    tabs: Rc<VecModel<TabInfo>>,
    /// Pages to open once the engine is up, and the index of the one to show.
    start_tabs: Vec<SessionTab>,
    start_active: usize,
    lazy_id_counter: Cell<u32>,
    dark_mode: Cell<bool>,

    /// `None` when no profile directory could be determined; nothing is persisted then.
    profile: Option<Profile>,
    settings: RefCell<Settings>,
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

fn format_timestamp(unix_secs: u64) -> String {
    let now = unix_now();
    if now >= unix_secs {
        let diff = now - unix_secs;
        if diff < 60 {
            return "Just now".to_string();
        } else if diff < 3600 {
            return format!("{}m ago", diff / 60);
        } else if diff < 86400 {
            return format!("{}h ago", diff / 3600);
        } else if diff < 86400 * 7 {
            return format!("{}d ago", diff / 86400);
        }
    }
    let days = unix_secs / 86400;
    format!("{days}d after 1970")
}

/// Decides what to open at startup: command-line pages, else the previous session,
/// else the home page. Returns the pages and the index of the active one.
fn startup_pages(
    cli_args: &[String],
    profile: Option<&Profile>,
    settings: &Settings,
) -> (Vec<SessionTab>, usize) {
    if !cli_args.is_empty() {
        let tabs = cli_args
            .iter()
            .map(|arg| SessionTab::new(input::resolve(arg, &settings.search_url), ""))
            .collect();
        return (tabs, 0);
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
    (vec![SessionTab::new(home, "")], 0)
}

impl BrowserApp {
    pub fn browser(&self) -> Option<BrowserLogic<'static>> {
        self.browser.upgrade()
    }

    pub fn webview(&self) -> Option<WebviewLogic<'static>> {
        self.webview.upgrade()
    }

    pub fn tokens(&self) -> Option<Tokens<'static>> {
        self.tokens.upgrade()
    }

    /// Wires every UI callback to the browser. The engine itself is created later,
    /// from [`Self::on_rendering_notifier`], once the GPU device exists.
    pub fn install<'a>(
        host: BrowserHost,
        browser: BrowserLogic<'a>,
        webview: WebviewLogic<'a>,
        tokens: Tokens<'a>,
        cli_args: Vec<String>,
        engine_factory: EngineFactory,
    ) -> Rc<Self> {
        let profile = Profile::default_location();
        let settings = profile.as_ref().map(Settings::load).unwrap_or_default();
        let bookmarks = profile.as_ref().map(Bookmarks::load).unwrap_or_default();
        let history = profile.as_ref().map(History::load).unwrap_or_default();
        let (start_tabs, start_active) = startup_pages(&cli_args, profile.as_ref(), &settings);
        let dark_mode = crate::theme::apply_theme(&tokens, &settings, true);

        let this = Rc::new_cyclic(|weak| Self {
            weak_self: weak.clone(),
            host,
            browser: browser.as_weak(),
            webview: webview.as_weak(),
            tokens: tokens.as_weak(),
            engine_factory,
            engine: RefCell::new(None),
            tab_list: RefCell::new(TabList::new()),
            tabs: Rc::new(VecModel::default()),
            start_tabs,
            start_active,
            lazy_id_counter: Cell::new(1_000_000),
            dark_mode: Cell::new(dark_mode),
            profile,
            settings: RefCell::new(settings),
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
        {
            let this = this.clone();
            browser.on_open_settings(move || this.open_settings());
        }
        {
            let this = this.clone();
            browser.on_close_settings(move || this.close_settings());
        }
        {
            let this = this.clone();
            browser.on_save_settings(move |home, search, restore, theme, preset, accent, radius| {
                this.save_settings(
                    home.as_str(),
                    search.as_str(),
                    restore,
                    theme as usize,
                    preset.as_str(),
                    accent.as_str(),
                    radius.as_str(),
                );
            });
        }
        {
            let this = this.clone();
            browser.on_select_search_preset(move |index| {
                this.select_search_preset(index as usize);
            });
        }
        {
            let this = this.clone();
            browser.on_clear_history(move || this.clear_history());
        }
        {
            let this = this.clone();
            browser.on_clear_bookmarks(move || this.clear_bookmarks());
        }
        {
            let this = this.clone();
            browser.on_clear_browsing_data(move || this.clear_browsing_data());
        }
        {
            let this = this.clone();
            browser.on_open_bookmarks(move || this.open_bookmarks());
        }
        {
            let this = this.clone();
            browser.on_close_bookmarks(move || this.close_bookmarks());
        }
        {
            let this = this.clone();
            browser.on_filter_bookmarks(move |query| this.filter_bookmarks(query.as_str()));
        }
        {
            let this = this.clone();
            browser.on_open_bookmark(move |url| this.open_bookmark(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_open_bookmark_new_tab(move |url| this.open_bookmark_new_tab(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_remove_bookmark(move |url| this.remove_bookmark(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_open_history(move || this.open_history());
        }
        {
            let this = this.clone();
            browser.on_close_history(move || this.close_history());
        }
        {
            let this = this.clone();
            browser.on_filter_history(move |query| this.filter_history(query.as_str()));
        }
        {
            let this = this.clone();
            browser.on_open_history_item(move |url| this.open_history_item(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_open_history_new_tab(move |url| this.open_history_new_tab(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_remove_history_item(move |url| this.remove_history_item(url.as_str()));
        }
        {
            let this = this.clone();
            browser.on_clear_all_history(move || this.clear_all_history());
        }

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
        let (width, height) = match self.webview() {
            Some(w) => (
                w.get_viewport_width().max(800.0),
                w.get_viewport_height().max(600.0),
            ),
            None => (800.0, 600.0),
        };
        let scale = self.host.scale_factor();

        eprintln!("nitium: Initializing web engine ({width}x{height} @ scale {scale})...");
        let weak = Rc::downgrade(self);
        let engine = (self.engine_factory)(
            device.clone(),
            queue.clone(),
            width,
            height,
            scale,
            self.dark_mode.get(),
            Box::new(move |event| {
                if let Some(this) = weak.upgrade() {
                    this.handle_event(event);
                }
            }),
        );
        match engine {
            Ok(engine) => *self.engine.borrow_mut() = Some(engine),
            Err(err) => {
                eprintln!("nitium: Failed to initialize web engine: {err}");
                return;
            }
        }

        if self.start_tabs.is_empty() {
            self.new_tab();
        } else {
            let active_idx = self.start_active.min(self.start_tabs.len() - 1);
            let mut active_tab_id = None;
            for (index, tab) in self.start_tabs.iter().enumerate() {
                if index == active_idx {
                    let live_id = self
                        .engine
                        .borrow()
                        .as_ref()
                        .map(|e| e.open_tab(&tab.url, false));
                    if let Some(id) = live_id {
                        self.push_tab_row(id, &tab.url);
                        if !tab.title.is_empty() {
                            self.update_row(id, |entry| entry.title = tab.title.clone());
                        }
                        active_tab_id = Some(id);
                    }
                } else {
                    let lazy_id = self.next_lazy_id();
                    self.push_lazy_tab_row(lazy_id, &tab.url, &tab.title);
                }
            }
            if let Some(id) = active_tab_id {
                self.select_tab(id);
            }
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
            let session = self.tab_list.borrow().to_session();
            if let Err(err) = session.save(profile) {
                eprintln!("nitium: cannot save session: {err}");
            }
        }
    }

    // -- Engine events ------------------------------------------------------

    fn handle_event(&self, event: BrowserEvent) {
        match event {
            BrowserEvent::Frame(image) => {
                if let Some(w) = self.webview() {
                    w.set_web_content(image);
                }
                self.host.request_redraw();
            }
            BrowserEvent::TabOpened { tab } => {
                // Opened by a page: its URL is not known yet, but it is not for typing into.
                self.push_tab_row(tab, BLANK_URL);
                self.show_tab(tab, false);
            }
            BrowserEvent::TabClosed { tab } => self.close_tab(tab),
            BrowserEvent::Url { tab, url } => {
                // A different page has no use for the previous page's search results.
                let navigated = self
                    .tab_list
                    .borrow()
                    .get(tab)
                    .is_some_and(|entry| {
                        entry.active
                            && input::without_fragment(&entry.url)
                                != input::without_fragment(&url)
                    });
                if navigated {
                    self.close_find();
                }
                self.history.borrow_mut().record_visit(&url, unix_now());
                self.history_dirty.set(true);
                self.update_row(tab, |entry| entry.url = url);
            }
            BrowserEvent::Title { tab, title } => {
                let title = title.unwrap_or_default();
                let current_url = self
                    .tab_list
                    .borrow()
                    .get(tab)
                    .map(|entry| entry.url.clone());
                if let Some(url) = current_url {
                    self.history.borrow_mut().set_title(&url, &title);
                    self.history_dirty.set(true);
                }
                self.update_row(tab, |entry| entry.title = title);
            }
            BrowserEvent::InputMethod { tab, field } => {
                if self.is_active(tab) {
                    self.set_ime_field(field);
                }
            }
            BrowserEvent::Cursor { tab, cursor } => {
                if self.is_active(tab) {
                    if let Some(w) = self.webview() {
                        w.set_cursor(cursor.into());
                    }
                }
            }
            BrowserEvent::Dialog { tab, dialog } => self.queue_dialog(tab, dialog),
            BrowserEvent::ContextMenu { tab, menu } => self.open_menu(tab, menu),
            BrowserEvent::SelectPopup { tab, select } => self.open_select(tab, select),
            BrowserEvent::ControlHidden { id, .. } => self.control_hidden(id),
            BrowserEvent::Loading { tab, loading } => {
                self.update_row(tab, |entry| entry.loading = loading);
            }
            BrowserEvent::History {
                tab,
                can_go_back,
                can_go_forward,
            } => {
                self.update_row(tab, |entry| {
                    entry.can_go_back = can_go_back;
                    entry.can_go_forward = can_go_forward;
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
        let is_only_tab = self.push_tab_row(tab, url) == 1;
        if activate || is_only_tab {
            self.select_tab(tab);
        }
    }

    fn next_lazy_id(&self) -> TabId {
        let id = self.lazy_id_counter.get();
        self.lazy_id_counter.set(id + 1);
        id
    }

    fn push_tab_row(&self, tab: TabId, url: &str) -> usize {
        let mut list = self.tab_list.borrow_mut();
        list.push(tab, url);
        let entry = list.get(tab).expect("just pushed");
        self.tabs.push(to_tab_info(entry));
        list.len()
    }

    fn push_lazy_tab_row(&self, tab: TabId, url: &str, title: &str) -> usize {
        let mut list = self.tab_list.borrow_mut();
        list.push_lazy(tab, url, title);
        let entry = list.get(tab).expect("just pushed");
        self.tabs.push(to_tab_info(entry));
        list.len()
    }

    fn select_tab(&self, tab: TabId) {
        let is_loaded = self.tab_list.borrow().is_loaded(tab);
        let actual_id = if !is_loaded {
            let (url, _title) = self
                .tab_list
                .borrow()
                .get(tab)
                .map(|e| (e.url.clone(), e.title.clone()))
                .unwrap_or_default();
            let live_id = self
                .engine
                .borrow()
                .as_ref()
                .map(|e| e.open_tab(&url, false));
            if let Some(live_id) = live_id {
                let mut list = self.tab_list.borrow_mut();
                list.replace_id(tab, live_id);
                list.mark_loaded(live_id);
                drop(list);
                for i in 0..self.tabs.row_count() {
                    let mut row = self.tabs.row_data(i).expect("row in range");
                    if row.id == tab as i32 {
                        row.id = live_id as i32;
                        self.tabs.set_row_data(i, row);
                        break;
                    }
                }
                live_id
            } else {
                tab
            }
        } else {
            tab
        };
        self.show_tab(actual_id, true);
    }

    /// Brings `tab` to the front. A blank tab gets the address bar's focus only when
    /// `address_if_blank` (tabs the user opened); any other tab focuses the page.
    fn show_tab(&self, tab: TabId, address_if_blank: bool) {
        let Some(index) = self.tab_list.borrow_mut().select(tab) else {
            return;
        };
        // Per-page state of the tab being left must not leak into the next one.
        self.close_find();
        self.dismiss_popup();
        self.set_ime_field(None);
        if let Some(w) = self.webview() {
            w.set_cursor("default".into());
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

        let row = self.tabs.row_data(index).expect("row in range");
        if let Some(browser) = self.browser() {
            if address_if_blank && input::display_url(row.url.as_str()).is_empty() {
                browser.invoke_focus_address();
            } else {
                browser.invoke_focus_content();
            }
        }
    }

    fn close_tab(&self, tab: TabId) {
        let is_loaded = self.tab_list.borrow().is_loaded(tab);
        let Some(index) = self.tab_list.borrow().index_of(tab) else {
            return;
        };
        let outcome = self.tab_list.borrow_mut().close(tab);

        // Questions from a closed page can no longer be answered; dropping them dismisses them.
        self.dialogs.borrow_mut().retain(|(owner, _)| *owner != tab);
        if is_loaded {
            self.with_engine(|engine| engine.close_tab(tab));
        }
        self.tabs.remove(index);

        match outcome {
            CloseOutcome::NotFound => {}
            CloseOutcome::LastClosed => {
                let _ = slint::quit_event_loop();
            }
            CloseOutcome::StayedActive => {}
            CloseOutcome::SelectNext(next_id) => {
                self.select_tab(next_id);
            }
        }
    }

    fn select_relative(&self, delta: isize) {
        let next = self.tab_list.borrow().relative_id(delta);
        if let Some(id) = next {
            self.select_tab(id);
        }
    }

    fn select_index(&self, index: usize) {
        let next = self.tab_list.borrow().id_at(index);
        if let Some(id) = next {
            self.select_tab(id);
        }
    }

    fn reopen_closed_tab(&self) {
        let url = self.tab_list.borrow_mut().pop_closed_url();
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
        let url = input::resolve(text, &self.settings.borrow().search_url);
        self.with_engine(|engine| engine.load_url(&url));
        self.clear_suggestions();
        if let Some(browser) = self.browser() {
            browser.invoke_focus_content();
        }
    }

    // -- Bookmarks and suggestions -------------------------------------------

    fn toggle_bookmark(&self) {
        let active = self
            .tab_list
            .borrow()
            .active()
            .map(|entry| (entry.url.clone(), entry.title.clone()));
        let Some((url, title)) = active else {
            return;
        };
        self.bookmarks
            .borrow_mut()
            .toggle(&url, &title, unix_now());
        if let Some(profile) = &self.profile {
            if let Err(err) = self.bookmarks.borrow().save(profile) {
                eprintln!("nitium: cannot save bookmarks: {err}");
            }
        }
        self.sync_active(false);
        if let Some(browser) = self.browser() {
            if browser.get_bookmarks_open() {
                let filter = browser.get_bookmarks_filter();
                self.populate_bookmarks(filter.as_str());
            }
        }
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
        if let Some(browser) = self.browser() {
            browser.set_suggestions(ModelRc::new(VecModel::from(suggestions)));
            browser.set_suggestion_index(-1);
        }
    }

    // -- Shortcuts ----------------------------------------------------------

    /// Returns true when the key was consumed by the application.
    fn handle_shortcut(&self, event: &slint::language::KeyEvent) -> bool {
        // Escape in the address bar abandons the edit and returns to the page.
        if event.text == SharedString::from(Key::Escape) {
            let Some(browser) = self.browser() else {
                return false;
            };
            if browser.get_bookmarks_open() {
                self.close_bookmarks();
                return true;
            }
            if browser.get_history_open() {
                self.close_history();
                return true;
            }
            if browser.get_settings_open() {
                self.close_settings();
                return true;
            }
            if browser.get_menu_visible() {
                self.menu_dismiss();
                return true;
            }
            if browser.get_address_focused() {
                self.clear_suggestions();
                self.sync_active(true);
                browser.invoke_focus_content();
                return true;
            }
            if browser.get_find_focused() {
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
                if let Some(id) = self.active_tab_id() {
                    self.close_tab(id);
                }
            }
            Shortcut::ReopenClosedTab => self.reopen_closed_tab(),
            Shortcut::FocusAddress => {
                if let Some(browser) = self.browser() {
                    browser.invoke_focus_address();
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
                let last = self.tab_list.borrow().last_id();
                if let Some(id) = last {
                    self.select_tab(id);
                }
            }
            Shortcut::Settings => self.toggle_settings(),
            Shortcut::Bookmarks => self.toggle_bookmarks(),
            Shortcut::History => self.toggle_history(),
        }
        true
    }

    // -- Input method -----------------------------------------------------------

    /// A text field in the page gained (`Some`) or lost (`None`) focus.
    fn set_ime_field(&self, field: Option<DeviceRect>) {
        if let Some(webview) = self.webview() {
            match field {
                Some(area) => {
                    // The engine reports device pixels; the UI works in logical ones.
                    let scale = self.host.scale_factor();
                    webview.set_ime_x(area.x / scale);
                    webview.set_ime_y(area.y / scale);
                    webview.set_ime_width(area.width / scale);
                    webview.set_ime_height(area.height / scale);
                    webview.set_ime_active(true);
                }
                None => {
                    webview.set_ime_active(false);
                    if self.ime.borrow_mut().reset() {
                        self.with_engine(|engine| engine.ime_dismiss());
                    }
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
        let Some(browser) = self.browser() else {
            return;
        };
        browser.set_find_open(true);
        // The bar only becomes visible, hence focusable, once this change has been applied.
        let weak_browser = self.browser.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(browser) = weak_browser.upgrade() {
                browser.invoke_focus_find();
            }
        });
        let query = browser.get_find_text();
        if !query.is_empty() {
            self.find_search(query.as_str());
        }
    }

    fn close_find(&self) {
        let Some(browser) = self.browser() else {
            return;
        };
        if !browser.get_find_open() {
            return;
        }
        browser.set_find_open(false);
        browser.set_find_status(SharedString::new());
        self.run_find(FindCommand::Clear);
        browser.invoke_focus_content();
    }

    fn find_search(&self, query: &str) {
        if query.is_empty() {
            if let Some(browser) = self.browser() {
                browser.set_find_status(SharedString::new());
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
        let is_open = self.browser().map_or(false, |b| b.get_find_open());
        if !is_open {
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
            engine.evaluate_script(
                &script,
                Box::new(move |answer| {
                    if !wants_status {
                        return;
                    }
                    if let Some(this) = weak.upgrade() {
                        this.show_find_status(answer);
                    }
                }),
            );
        });
    }

    fn show_find_status(&self, answer: Option<String>) {
        let Some(browser) = self.browser() else {
            return;
        };
        let text = match answer {
            // The script could not run (page still loading, …): show nothing.
            None => String::new(),
            _ if !browser.get_find_open() || browser.get_find_text().is_empty() => {
                String::new()
            }
            Some(answer) => {
                let status = find::parse_status(&answer);
                if status.total == 0 {
                    "No matches".to_string()
                } else {
                    format!("{}/{}", status.current, status.total)
                }
            }
        };
        browser.set_find_status(text.into());
    }

    // -- Questions from the page ------------------------------------------------------

    fn active_tab_id(&self) -> Option<TabId> {
        self.tab_list.borrow().active_id()
    }

    fn queue_dialog(&self, tab: TabId, dialog: PageDialog) {
        self.dialogs.borrow_mut().push((tab, dialog));
        let showing = self.browser().map_or(false, |b| b.get_dialog_visible());
        if !showing {
            self.refresh_dialog();
        }
    }

    /// Shows the oldest question asked by the active tab, or hides the dialog if there is none.
    fn refresh_dialog(&self) {
        let Some(browser) = self.browser() else {
            return;
        };
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
            browser.set_dialog_visible(false);
            return;
        };
        browser.set_dialog_kind(kind);
        browser.set_dialog_title(title.into());
        browser.set_dialog_message(message.into());
        browser.set_dialog_input(initial.into());
        browser.set_dialog_password(SharedString::new());
        browser.set_dialog_visible(true);
        // The dialog only becomes visible, hence focusable, once this change has been applied.
        let weak_browser = self.browser.clone();
        slint::Timer::single_shot(Duration::ZERO, move || {
            if let Some(browser) = weak_browser.upgrade() {
                browser.invoke_focus_dialog();
            }
        });
    }

    /// OK / Allow / Sign in (`accept`) or Cancel / Deny for the dialog on screen.
    fn answer_dialog(&self, accept: bool) {
        let Some(browser) = self.browser() else {
            return;
        };
        let active = self.active_tab_id();
        let dialog = {
            let mut dialogs = self.dialogs.borrow_mut();
            let position = dialogs.iter().position(|(tab, _)| Some(*tab) == active);
            position.map(|position| dialogs.remove(position).1)
        };
        let Some(dialog) = dialog else {
            browser.set_dialog_visible(false);
            return;
        };
        let input = browser.get_dialog_input();
        let password = browser.get_dialog_password();
        // Answered with no borrow held: the engine may react by raising new events.
        if accept {
            dialog.accept(input.as_str(), password.as_str());
        } else {
            dialog.cancel();
        }
        browser.set_dialog_password(SharedString::new());
        self.refresh_dialog();
        if !browser.get_dialog_visible() {
            browser.invoke_focus_content();
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
        let Some(browser) = self.browser() else {
            return;
        };
        // Rows are 28px (9px for separators) plus 4px padding above and below.
        let height: f32 = 8.0
            + rows
                .iter()
                .map(|row| if row.separator { 9.0 } else { 28.0 })
                .sum::<f32>();
        self.menu_rows.set_vec(rows);
        browser.set_menu_x(x);
        browser.set_menu_y(y);
        browser.set_menu_width(min_width);
        browser.set_menu_height(height);
        browser.set_menu_visible(true);
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
        if let Some(browser) = self.browser() {
            browser.set_menu_visible(false);
        }
    }

    fn focus_page(&self) {
        if let Some(browser) = self.browser() {
            browser.invoke_focus_content();
        }
    }

    // -- Helpers ------------------------------------------------------------

    fn is_active(&self, tab: TabId) -> bool {
        self.tab_list.borrow().is_active(tab)
    }

    fn with_engine(&self, f: impl FnOnce(&dyn WebEngine)) {
        if let Some(engine) = self.engine.borrow().as_deref() {
            f(engine);
        }
    }

    fn scale_factor(&self) -> f32 {
        self.host.scale_factor()
    }

    /// Edits a tab's entry in `tab_list`, mirrors it to `tabs`, and refreshes the toolbar
    /// if it is the active tab.
    fn update_row(&self, tab: TabId, edit: impl FnOnce(&mut TabEntry)) {
        let updated = {
            let mut list = self.tab_list.borrow_mut();
            list.update(tab, edit)
                .map(|(index, entry)| (index, to_tab_info(entry), entry.active))
        };
        let Some((index, row, active)) = updated else {
            return;
        };
        self.tabs.set_row_data(index, row);
        if active {
            self.sync_active(false);
        }
    }

    /// Mirrors the active tab into the toolbar, address bar and window title.
    /// The address is left alone while the user is typing unless `force_address`.
    fn sync_active(&self, force_address: bool) {
        let active_entry = self.tab_list.borrow().active().cloned();
        let Some(entry) = active_entry else {
            return;
        };
        let Some(browser) = self.browser() else {
            return;
        };
        browser.set_can_go_back(entry.can_go_back);
        browser.set_can_go_forward(entry.can_go_forward);
        browser.set_loading(entry.loading);
        let page_zoom = self
            .engine
            .borrow()
            .as_ref()
            .map_or(zoom::DEFAULT, |engine| engine.zoom());
        browser.set_zoom_percent(zoom::percent(page_zoom));
        browser.set_bookmarked(self.bookmarks.borrow().contains(&entry.url));
        browser.set_window_title(if entry.title.is_empty() {
            "Nitium".into()
        } else {
            format!("{} — Nitium", entry.title).into()
        });
        if force_address || !browser.get_address_focused() {
            browser.set_address(input::display_url(&entry.url).into());
        }
    }

    // -- Settings ------------------------------------------------------------

    fn open_settings(&self) {
        let Some(browser) = self.browser() else {
            return;
        };
        let settings = self.settings.borrow();
        browser.set_settings_home_url(settings.home_url.as_str().into());
        browser.set_settings_search_url(settings.search_url.as_str().into());
        browser.set_settings_search_preset(settings.search_preset_index().unwrap_or(6) as i32);
        browser.set_settings_restore_session(settings.restore_session);
        browser.set_settings_theme(settings.theme.as_index() as i32);
        browser.set_settings_theme_preset(settings.theme_preset.as_str().into());
        browser.set_settings_accent_color(
            settings
                .accent_color
                .as_deref()
                .unwrap_or("default")
                .into(),
        );
        browser.set_settings_corner_radius(settings.corner_radius.as_str().into());
        browser.set_settings_status("".into());
        browser.set_settings_open(true);
    }

    fn close_settings(&self) {
        if let Some(browser) = self.browser() {
            browser.set_settings_open(false);
        }
    }

    fn toggle_settings(&self) {
        let is_open = self.browser().map_or(false, |b| b.get_settings_open());
        if is_open {
            self.close_settings();
        } else {
            self.open_settings();
        }
    }

    fn select_search_preset(&self, index: usize) {
        let Some(browser) = self.browser() else {
            return;
        };
        if let Some(preset) = SEARCH_PRESETS.get(index) {
            browser.set_settings_search_url(preset.url_template.into());
            browser.set_settings_search_preset(index as i32);
        } else {
            browser.set_settings_search_preset(6); // Custom
        }
    }

    fn save_settings(
        &self,
        home_url: &str,
        search_url: &str,
        restore_session: bool,
        theme_index: usize,
        theme_preset: &str,
        accent_color: &str,
        corner_radius: &str,
    ) {
        let mut settings = self.settings.borrow_mut();
        settings.home_url = home_url.to_string();
        settings.search_url = search_url.to_string();
        settings.restore_session = restore_session;
        settings.theme = ThemeMode::from_index(theme_index);
        settings.theme_preset = theme_preset.to_string();
        settings.accent_color = if accent_color == "default" || accent_color.is_empty() {
            None
        } else {
            Some(accent_color.to_string())
        };
        settings.corner_radius = corner_radius.to_string();
        let sanitized = settings.clone().sanitized();
        *settings = sanitized;

        if let Some(profile) = self.profile.as_ref() {
            if let Err(err) = settings.save(profile) {
                eprintln!("nitium: cannot save settings: {err}");
            }
        }

        let is_dark = if let Some(tokens) = self.tokens() {
            crate::theme::apply_theme(&tokens, &settings, true)
        } else {
            settings.theme == ThemeMode::Dark
        };
        self.dark_mode.set(is_dark);
        drop(settings);
        self.with_engine(|engine| engine.set_theme(is_dark));

        if let Some(browser) = self.browser() {
            browser.set_settings_status("Settings saved.".into());
        }
    }

    fn clear_history(&self) {
        self.history.borrow_mut().clear();
        self.history_dirty.set(true);
        if let Some(profile) = self.profile.as_ref() {
            if let Err(err) = self.history.borrow().save(profile) {
                eprintln!("nitium: cannot save history: {err}");
            }
        }
        if let Some(browser) = self.browser() {
            browser.set_settings_status("Browsing history cleared.".into());
            if browser.get_history_open() {
                let filter = browser.get_history_filter();
                self.populate_history(filter.as_str());
            }
        }
    }

    fn clear_bookmarks(&self) {
        self.bookmarks.borrow_mut().clear();
        if let Some(profile) = self.profile.as_ref() {
            if let Err(err) = self.bookmarks.borrow().save(profile) {
                eprintln!("nitium: cannot save bookmarks: {err}");
            }
        }
        self.sync_active(false);
        if let Some(browser) = self.browser() {
            browser.set_settings_status("Bookmarks cleared.".into());
            if browser.get_bookmarks_open() {
                let filter = browser.get_bookmarks_filter();
                self.populate_bookmarks(filter.as_str());
            }
        }
    }

    fn clear_browsing_data(&self) {
        self.with_engine(|engine| engine.clear_browsing_data());
        if let Some(browser) = self.browser() {
            browser.set_settings_status("Cookies and cache cleared.".into());
        }
    }

    // -- Bookmarks overlay ---------------------------------------------------

    fn open_bookmarks(&self) {
        let Some(browser) = self.browser() else {
            return;
        };
        browser.set_bookmarks_filter("".into());
        self.populate_bookmarks("");
        browser.set_bookmarks_open(true);
    }

    fn close_bookmarks(&self) {
        if let Some(browser) = self.browser() {
            browser.set_bookmarks_open(false);
        }
    }

    fn toggle_bookmarks(&self) {
        let is_open = self.browser().map_or(false, |b| b.get_bookmarks_open());
        if is_open {
            self.close_bookmarks();
        } else {
            self.open_bookmarks();
        }
    }

    fn filter_bookmarks(&self, query: &str) {
        self.populate_bookmarks(query);
    }

    fn populate_bookmarks(&self, query: &str) {
        let Some(browser) = self.browser() else {
            return;
        };
        let b = self.bookmarks.borrow();
        let items: Vec<BookmarkItem> = if query.trim().is_empty() {
            b.items()
                .iter()
                .map(|entry| BookmarkItem {
                    url: entry.url.as_str().into(),
                    title: entry.title.as_str().into(),
                })
                .collect()
        } else {
            b.search(query, 100)
                .into_iter()
                .map(|entry| BookmarkItem {
                    url: entry.url.as_str().into(),
                    title: entry.title.as_str().into(),
                })
                .collect()
        };
        browser.set_bookmarks_list(ModelRc::new(VecModel::from(items)));
    }

    fn open_bookmark(&self, url: &str) {
        self.close_bookmarks();
        self.navigate(url);
    }

    fn open_bookmark_new_tab(&self, url: &str) {
        self.close_bookmarks();
        self.open_tab(url, true);
    }

    fn remove_bookmark(&self, url: &str) {
        self.bookmarks.borrow_mut().remove(url);
        if let Some(profile) = &self.profile {
            if let Err(err) = self.bookmarks.borrow().save(profile) {
                eprintln!("nitium: cannot save bookmarks: {err}");
            }
        }
        self.sync_active(false);
        if let Some(browser) = self.browser() {
            let filter = browser.get_bookmarks_filter();
            self.populate_bookmarks(filter.as_str());
        }
    }

    // -- History overlay -----------------------------------------------------

    fn open_history(&self) {
        let Some(browser) = self.browser() else {
            return;
        };
        browser.set_history_filter("".into());
        self.populate_history("");
        browser.set_history_open(true);
    }

    fn close_history(&self) {
        if let Some(browser) = self.browser() {
            browser.set_history_open(false);
        }
    }

    fn toggle_history(&self) {
        let is_open = self.browser().map_or(false, |b| b.get_history_open());
        if is_open {
            self.close_history();
        } else {
            self.open_history();
        }
    }

    fn filter_history(&self, query: &str) {
        self.populate_history(query);
    }

    fn populate_history(&self, query: &str) {
        let Some(browser) = self.browser() else {
            return;
        };
        let h = self.history.borrow();
        let items: Vec<HistoryItem> = if query.trim().is_empty() {
            h.recent(200)
                .into_iter()
                .map(|entry| HistoryItem {
                    url: entry.url.as_str().into(),
                    title: entry.title.as_str().into(),
                    date_label: format_timestamp(entry.last_visit).into(),
                    visits: entry.visits as i32,
                })
                .collect()
        } else {
            h.suggest(query, 100)
                .into_iter()
                .map(|entry| HistoryItem {
                    url: entry.url.as_str().into(),
                    title: entry.title.as_str().into(),
                    date_label: format_timestamp(entry.last_visit).into(),
                    visits: entry.visits as i32,
                })
                .collect()
        };
        browser.set_history_list(ModelRc::new(VecModel::from(items)));
    }

    fn open_history_item(&self, url: &str) {
        self.close_history();
        self.navigate(url);
    }

    fn open_history_new_tab(&self, url: &str) {
        self.close_history();
        self.open_tab(url, true);
    }

    fn remove_history_item(&self, url: &str) {
        self.history.borrow_mut().remove(url);
        self.history_dirty.set(true);
        if let Some(browser) = self.browser() {
            let filter = browser.get_history_filter();
            self.populate_history(filter.as_str());
        }
    }

    fn clear_all_history(&self) {
        self.clear_history();
        if let Some(browser) = self.browser() {
            let filter = browser.get_history_filter();
            self.populate_history(filter.as_str());
        }
    }
}

fn to_tab_info(entry: &TabEntry) -> TabInfo {
    TabInfo {
        id: entry.id as i32,
        title: entry.title.as_str().into(),
        url: entry.url.as_str().into(),
        loading: entry.loading,
        active: entry.active,
        can_go_back: entry.can_go_back,
        can_go_forward: entry.can_go_forward,
    }
}
