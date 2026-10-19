//! Pure tab-strip model: open tabs, active selection, relative/indexed switching,
//! and the recently-closed URL stack.

use crate::controls::TabId;
use crate::input;
use crate::session::Session;

const MAX_CLOSED_TABS: usize = 25;

/// State of a single open tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabEntry {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub loading: bool,
    pub active: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    /// Whether this tab's web engine WebView has been created.
    pub loaded: bool,
}

impl TabEntry {
    pub fn new(id: TabId, url: impl Into<String>) -> Self {
        Self {
            id,
            title: String::new(),
            url: url.into(),
            loading: false,
            active: false,
            can_go_back: false,
            can_go_forward: false,
            loaded: true,
        }
    }

    pub fn new_lazy(id: TabId, url: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            url: url.into(),
            loading: false,
            active: false,
            can_go_back: false,
            can_go_forward: false,
            loaded: false,
        }
    }
}

/// What happened when a tab was closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseOutcome {
    /// The tab was not found.
    NotFound,
    /// The last remaining tab was closed (the browser should exit).
    LastClosed,
    /// A background tab was closed; the active tab did not change.
    StayedActive,
    /// The active tab was closed; `next` should now be selected.
    SelectNext(TabId),
}

/// Ordered list of open tabs plus the stack of recently closed web URLs.
#[derive(Debug, Default, Clone)]
pub struct TabList {
    tabs: Vec<TabEntry>,
    closed_urls: Vec<String>,
}

impl TabList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[TabEntry] {
        &self.tabs
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    pub fn get(&self, id: TabId) -> Option<&TabEntry> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn active_index(&self) -> Option<usize> {
        self.tabs.iter().position(|t| t.active)
    }

    pub fn active(&self) -> Option<&TabEntry> {
        self.tabs.iter().find(|t| t.active)
    }

    pub fn active_id(&self) -> Option<TabId> {
        self.active().map(|t| t.id)
    }

    pub fn is_active(&self, id: TabId) -> bool {
        self.get(id).is_some_and(|t| t.active)
    }

    /// Appends a new inactive tab at the end of the strip.
    pub fn push(&mut self, id: TabId, url: impl Into<String>) {
        self.tabs.push(TabEntry::new(id, url));
    }

    /// Appends an unloaded lazy tab (title and URL known, WebView not yet created).
    pub fn push_lazy(&mut self, id: TabId, url: impl Into<String>, title: impl Into<String>) {
        self.tabs.push(TabEntry::new_lazy(id, url, title));
    }

    pub fn is_loaded(&self, id: TabId) -> bool {
        self.get(id).is_some_and(|t| t.loaded)
    }

    pub fn mark_loaded(&mut self, id: TabId) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id) {
            tab.loaded = true;
        }
    }

    /// Replaces the tab id (e.g. upgrading a placeholder id to a live engine id).
    pub fn replace_id(&mut self, old_id: TabId, new_id: TabId) -> bool {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == old_id) {
            tab.id = new_id;
            true
        } else {
            false
        }
    }

    /// Marks `id` as the active tab and clears `active` on all others.
    /// Returns the index of `id` if it exists.
    pub fn select(&mut self, id: TabId) -> Option<usize> {
        let index = self.index_of(id)?;
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            tab.active = i == index;
        }
        Some(index)
    }

    /// Mutates the tab `id` in place and returns its index and updated state.
    pub fn update(&mut self, id: TabId, edit: impl FnOnce(&mut TabEntry)) -> Option<(usize, &TabEntry)> {
        let index = self.index_of(id)?;
        edit(&mut self.tabs[index]);
        Some((index, &self.tabs[index]))
    }

    /// Removes `id`, records its URL on the closed-tab stack if it is a web URL,
    /// and reports what the caller should do next.
    pub fn close(&mut self, id: TabId) -> CloseOutcome {
        let Some(index) = self.index_of(id) else {
            return CloseOutcome::NotFound;
        };
        let removed = self.tabs.remove(index);
        if input::is_web_url(&removed.url) {
            self.closed_urls.push(removed.url);
            if self.closed_urls.len() > MAX_CLOSED_TABS {
                self.closed_urls.remove(0);
            }
        }
        if self.tabs.is_empty() {
            return CloseOutcome::LastClosed;
        }
        if removed.active {
            let next_index = index.min(self.tabs.len() - 1);
            CloseOutcome::SelectNext(self.tabs[next_index].id)
        } else {
            CloseOutcome::StayedActive
        }
    }

    /// Returns the `TabId` at `(active + delta) mod len`, wrapping around.
    pub fn relative_id(&self, delta: isize) -> Option<TabId> {
        let count = self.tabs.len() as isize;
        let active = self.active_index()? as isize;
        let next = (active + delta).rem_euclid(count) as usize;
        self.tabs.get(next).map(|t| t.id)
    }

    /// Returns the `TabId` at zero-based display `index`.
    pub fn id_at(&self, index: usize) -> Option<TabId> {
        self.tabs.get(index).map(|t| t.id)
    }

    /// Returns the `TabId` of the last tab in the strip.
    pub fn last_id(&self) -> Option<TabId> {
        self.tabs.last().map(|t| t.id)
    }

    /// Pops the most recently closed web URL, if any.
    pub fn pop_closed_url(&mut self) -> Option<String> {
        self.closed_urls.pop()
    }

    /// Builds a persistent [`Session`] snapshot of the current tabs including titles.
    pub fn to_session(&self) -> Session {
        let entries: Vec<(String, String)> = self
            .tabs
            .iter()
            .map(|t| (t.url.clone(), t.title.clone()))
            .collect();
        Session::from_entries(&entries, self.active_index().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_active_tab_picks_right_neighbour_or_left_at_end() {
        let mut list = TabList::new();
        list.push(10, "https://a.example/");
        list.push(20, "https://b.example/");
        list.push(30, "https://c.example/");
        list.select(20);

        assert_eq!(list.close(20), CloseOutcome::SelectNext(30));
        list.select(30);
        assert_eq!(list.close(30), CloseOutcome::SelectNext(10));
        list.select(10);
        assert_eq!(list.close(10), CloseOutcome::LastClosed);
    }

    #[test]
    fn relative_selection_wraps_around() {
        let mut list = TabList::new();
        list.push(1, "https://a.example/");
        list.push(2, "https://b.example/");
        list.push(3, "https://c.example/");
        list.select(1);

        assert_eq!(list.relative_id(-1), Some(3));
        assert_eq!(list.relative_id(1), Some(2));
        list.select(3);
        assert_eq!(list.relative_id(1), Some(1));
    }

    #[test]
    fn reopen_stack_is_lifo_and_ignores_blank_tabs() {
        let mut list = TabList::new();
        list.push(1, "about:blank");
        list.push(2, "https://first.example/");
        list.push(3, "https://second.example/");

        list.close(1);
        list.close(2);
        list.close(3);

        assert_eq!(list.pop_closed_url().as_deref(), Some("https://second.example/"));
        assert_eq!(list.pop_closed_url().as_deref(), Some("https://first.example/"));
        assert_eq!(list.pop_closed_url(), None);
    }

    #[test]
    fn lazy_tab_lifecycle() {
        let mut list = TabList::new();
        list.push_lazy(100, "https://lazy.example/", "Lazy Page");
        assert!(!list.is_loaded(100));
        assert_eq!(list.get(100).unwrap().title, "Lazy Page");

        list.mark_loaded(100);
        assert!(list.is_loaded(100));

        assert!(list.replace_id(100, 200));
        assert_eq!(list.get(200).unwrap().title, "Lazy Page");
        assert!(!list.replace_id(999, 300));
    }

    #[test]
    fn session_stores_titles() {
        let mut list = TabList::new();
        list.push_lazy(1, "https://a.test/", "Page Alpha");
        list.push_lazy(2, "https://b.test/", "Page Beta");
        list.select(2);

        let session = list.to_session();
        assert_eq!(session.tabs.len(), 2);
        assert_eq!(session.tabs[0].title, "Page Alpha");
        assert_eq!(session.tabs[1].title, "Page Beta");
        assert_eq!(session.active, 1);
    }
}
