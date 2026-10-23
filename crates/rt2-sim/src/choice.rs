//! Answers to the scenario's two-button choices (scenarios.md 2.4.1).
//!
//! In the original a choice (`rule+0x60 == 2`) opens a modal window inside `rule_evaluate`
//! (`rule_show_message` `0x455e70` -> `0x4510a0`): the game loop stops until the player clicks,
//! and the button picks the effect set before the evaluation goes on (button 2 = set 1). The
//! clone keeps that order exactly by replaying: in interactive mode [`World::advance`] keeps a
//! copy of the world; a choice with no answer yet throws the step away, restores the copy and
//! waits (the game is paused, [`World::pending_choice`]); [`Command::AnswerChoice`] records the
//! button and the next `advance` runs the same step again, which meets the same choice (the
//! simulation is deterministic) and takes the answer. Headless runs (the default) answer button
//! 0 at once, like a network game of the original.
//!
//! [`Command::AnswerChoice`]: crate::Command::AnswerChoice

use crate::scenario::Message;
use crate::world::World;

/// A choice waiting for the player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingChoice {
    /// Id to quote in [`crate::Command::AnswerChoice`].
    pub id: u32,
    pub message: Message,
}

/// Choice state of a world (outside the save).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    /// Wait for the player instead of answering button 0.
    pub(crate) interactive: bool,
    /// Answers of the step being (re)played, in the order the choices come up.
    pub(crate) answers: Vec<u8>,
    pub(crate) cursor: usize,
    /// Set while `advance` can throw the step away.
    pub(crate) replayable: bool,
    /// A choice came up with no answer during this step.
    pub(crate) unanswered: Option<Message>,
    pub(crate) pending: Option<PendingChoice>,
    pub(crate) next_id: u32,
}

impl Choices {
    /// The button for a choice that just came up (0 or 1).
    pub(crate) fn answer(&mut self, message: &Message) -> usize {
        if !self.interactive {
            return 0;
        }
        if let Some(&b) = self.answers.get(self.cursor) {
            self.cursor += 1;
            return usize::from(b.min(1));
        }
        if self.replayable && self.unanswered.is_none() {
            self.unanswered = Some(message.clone());
        }
        0
    }
}

impl World {
    /// Interactive mode: scenario choices wait for [`crate::Command::AnswerChoice`]. Off by
    /// default (button 0 at once, what the fidelity scoreboard compares).
    pub fn set_interactive_choices(&mut self, on: bool) {
        self.choices.interactive = on;
    }

    /// The choice the game waits for; while there is one, [`World::advance`] does nothing.
    pub fn pending_choice(&self) -> Option<&PendingChoice> {
        self.choices.pending.as_ref()
    }

    /// `Command::AnswerChoice`: button 0 (first) or 1 (second) for the pending choice `id`.
    pub(crate) fn answer_choice(&mut self, id: u32, button: u8) -> bool {
        match &self.choices.pending {
            Some(p) if p.id == id => {
                self.choices.pending = None;
                self.choices.answers.push(button.min(1));
                true
            }
            _ => false,
        }
    }
}
