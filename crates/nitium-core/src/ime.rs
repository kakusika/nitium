//! Input-method (IME) composition bookkeeping.
//!
//! The UI toolkit reports two things: the *preedit* (text still being converted, e.g.
//! `かんじ`) whenever it changes, and the *commit* (final text, e.g. `漢字`). The web
//! engine wants a `Start` / `Update` / `End` sequence. [`ImeState`] turns the former
//! into the latter.
//!
//! Toolkits do not agree on the order of "preedit cleared" and "text committed", so a
//! cleared preedit only *schedules* the end of the composition: the caller calls
//! [`ImeState::settle`] shortly afterwards, and the composition is cancelled only if no
//! commit arrived in between.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeAction {
    Start,
    Update(String),
    /// Ends the composition with the final text; empty means "cancelled".
    End(String),
}

#[derive(Debug, Default)]
pub struct ImeState {
    composing: bool,
    preedit_cleared: bool,
}

impl ImeState {
    pub fn is_composing(&self) -> bool {
        self.composing
    }

    /// The preedit text changed.
    pub fn preedit(&mut self, text: &str) -> Vec<ImeAction> {
        if text.is_empty() {
            if self.composing {
                self.preedit_cleared = true;
            }
            return Vec::new();
        }
        self.preedit_cleared = false;
        let mut actions = Vec::new();
        if !self.composing {
            self.composing = true;
            actions.push(ImeAction::Start);
        }
        actions.push(ImeAction::Update(text.to_string()));
        actions
    }

    /// Text was committed. It may arrive without any preedit (e.g. direct input).
    pub fn commit(&mut self, text: &str) -> Vec<ImeAction> {
        if text.is_empty() {
            return Vec::new();
        }
        let mut actions = Vec::new();
        if !self.composing {
            actions.push(ImeAction::Start);
        }
        actions.push(ImeAction::End(text.to_string()));
        self.composing = false;
        self.preedit_cleared = false;
        actions
    }

    /// Call shortly after a cleared preedit: cancels the composition if nothing was
    /// committed in the meantime.
    pub fn settle(&mut self) -> Vec<ImeAction> {
        if self.composing && self.preedit_cleared {
            self.composing = false;
            self.preedit_cleared = false;
            vec![ImeAction::End(String::new())]
        } else {
            Vec::new()
        }
    }

    /// The input method went away (focus left the field). Returns true if a
    /// composition was in progress and the engine must be told it was dismissed.
    pub fn reset(&mut self) -> bool {
        let was_composing = self.composing;
        self.composing = false;
        self.preedit_cleared = false;
        was_composing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ImeAction::*;

    fn s(text: &str) -> String {
        text.to_string()
    }

    #[test]
    fn typing_then_committing() {
        let mut ime = ImeState::default();
        assert_eq!(ime.preedit("か"), [Start, Update(s("か"))]);
        assert_eq!(ime.preedit("かん"), [Update(s("かん"))]);
        assert!(ime.is_composing());
        assert_eq!(ime.commit("漢"), [End(s("漢"))]);
        assert!(!ime.is_composing());
    }

    #[test]
    fn preedit_cleared_before_commit_does_not_cancel() {
        let mut ime = ImeState::default();
        ime.preedit("かん");
        assert!(ime.preedit("").is_empty());
        assert_eq!(ime.commit("漢"), [End(s("漢"))]);
        assert!(ime.settle().is_empty(), "already committed");
    }

    #[test]
    fn preedit_cleared_after_commit_is_harmless() {
        let mut ime = ImeState::default();
        ime.preedit("かん");
        ime.commit("漢");
        assert!(ime.preedit("").is_empty());
        assert!(ime.settle().is_empty());
        assert!(!ime.is_composing());
    }

    #[test]
    fn cancelled_composition_ends_empty() {
        let mut ime = ImeState::default();
        ime.preedit("かん");
        assert!(ime.preedit("").is_empty());
        assert_eq!(ime.settle(), [End(String::new())]);
        assert!(!ime.is_composing());
        assert!(ime.settle().is_empty(), "only once");
    }

    #[test]
    fn typing_again_before_settling_keeps_the_composition() {
        let mut ime = ImeState::default();
        ime.preedit("か");
        ime.preedit("");
        assert_eq!(ime.preedit("かな"), [Update(s("かな"))], "no second Start");
        assert!(ime.settle().is_empty());
        assert!(ime.is_composing());
    }

    #[test]
    fn direct_commit_without_preedit() {
        let mut ime = ImeState::default();
        assert_eq!(ime.commit("あ"), [Start, End(s("あ"))]);
        assert!(ime.commit("").is_empty());
    }

    #[test]
    fn reset_reports_interrupted_composition() {
        let mut ime = ImeState::default();
        assert!(!ime.reset());
        ime.preedit("か");
        assert!(ime.reset());
        assert!(!ime.is_composing());
        assert!(!ime.reset());
    }
}
