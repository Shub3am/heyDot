//! The turns a session remembers and when it forgets them: past ten turns, after ten idle minutes, on New chat.
//! Must not build requests or talk to the model; context.rs and session.rs do.

use std::time::{Duration, SystemTime};

use dot_providers::ChatMessage;

use crate::context::{Turn, UserInput, build_messages};

/// Counts the new question, so a request carries at most nine earlier turns.
const MAX_TURNS: usize = 10;
const IDLE_RESET: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Default)]
pub(crate) struct History {
    turns: Vec<Turn>,
    last_asked: Option<SystemTime>,
}

pub(crate) struct StartedTurn {
    pub(crate) messages: Vec<ChatMessage>,
    pub(crate) forgot_earlier_turns: bool,
}

impl History {
    /// Remembers the new question as the running turn and returns the request that asks it.
    pub(crate) fn start_turn(&mut self, input: UserInput, now: SystemTime) -> StartedTurn {
        // Wall-clock time, because Instant on macOS stops while the Mac sleeps. A clock set back counts as no time passed.
        let idle = self.last_asked.is_some_and(|last_asked| {
            now.duration_since(last_asked)
                .is_ok_and(|idle_for| idle_for >= IDLE_RESET)
        });
        let forgot_earlier_turns = idle && !self.turns.is_empty();
        if idle {
            self.turns.clear();
        }
        self.last_asked = Some(now);
        self.turns.retain(|turn| !turn.answer.is_empty());
        let excess = self.turns.len().saturating_sub(MAX_TURNS - 1);
        self.turns.drain(..excess);
        let running_turn = Turn {
            question: input.text.clone(),
            had_screenshot: input.jpeg_screenshot.is_some(),
            answer: String::new(),
        };
        let messages = build_messages(&self.turns, input);
        self.turns.push(running_turn);
        StartedTurn {
            messages,
            forgot_earlier_turns,
        }
    }

    pub(crate) fn append_to_answer(&mut self, text: &str) {
        self.turns
            .last_mut()
            .expect("an answer arrives only while its turn is running")
            .answer
            .push_str(text);
    }

    pub(crate) fn discard_running_turn(&mut self) {
        self.turns.pop();
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(text: &str) -> UserInput {
        UserInput {
            text: text.to_owned(),
            jpeg_screenshot: None,
        }
    }

    fn ask_and_answer(history: &mut History, text: &str, now: SystemTime) -> StartedTurn {
        let started = history.start_turn(question(text), now);
        history.append_to_answer(&format!("answer to {text}"));
        started
    }

    #[test]
    fn keeps_at_most_ten_turns_including_the_new_question() {
        let mut history = History::default();
        let start = SystemTime::now();
        for number in 1..=11 {
            ask_and_answer(
                &mut history,
                &format!("question {number}"),
                start + Duration::from_secs(number),
            );
        }

        let started = history.start_turn(question("question 12"), start + Duration::from_secs(12));

        assert_eq!(started.messages.len(), 1 + 9 * 2 + 1);
        assert_eq!(started.messages[1].text, "question 3");
        assert_eq!(started.messages[19].text, "question 12");
    }

    #[test]
    fn forgets_earlier_turns_after_ten_idle_minutes() {
        let mut history = History::default();
        let start = SystemTime::now();
        ask_and_answer(&mut history, "What city is this?", start);

        let started = history.start_turn(
            question("How many people live there?"),
            start + Duration::from_secs(600),
        );

        assert_eq!(started.messages.len(), 2);
        assert!(started.forgot_earlier_turns);
    }

    #[test]
    fn keeps_earlier_turns_within_ten_idle_minutes() {
        let mut history = History::default();
        let start = SystemTime::now();
        ask_and_answer(&mut history, "What city is this?", start);

        let started = history.start_turn(
            question("How many people live there?"),
            start + Duration::from_secs(599),
        );

        assert_eq!(started.messages.len(), 4);
        assert!(!started.forgot_earlier_turns);
    }

    #[test]
    fn a_clock_set_back_keeps_earlier_turns() {
        let mut history = History::default();
        let start = SystemTime::now();
        ask_and_answer(&mut history, "What city is this?", start);

        let started = history.start_turn(
            question("How many people live there?"),
            start - Duration::from_secs(3600),
        );

        assert_eq!(started.messages.len(), 4);
        assert!(!started.forgot_earlier_turns);
    }

    #[test]
    fn idle_minutes_count_from_the_last_question() {
        let mut history = History::default();
        let start = SystemTime::now();
        ask_and_answer(&mut history, "question 1", start);
        ask_and_answer(&mut history, "question 2", start + Duration::from_secs(500));

        let started = history.start_turn(question("question 3"), start + Duration::from_secs(1000));

        assert_eq!(started.messages.len(), 6);
    }

    #[test]
    fn the_first_question_forgot_nothing() {
        let started = History::default().start_turn(question("Hi"), SystemTime::now());

        assert!(!started.forgot_earlier_turns);
    }

    #[test]
    fn a_discarded_turn_is_left_out_of_the_next_question() {
        let mut history = History::default();
        let start = SystemTime::now();
        history.start_turn(question("What city is this?"), start);
        history.append_to_answer("Par");
        history.discard_running_turn();

        let started = history.start_turn(question("Try again?"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
    }

    #[test]
    fn a_question_left_without_any_answer_text_is_forgotten() {
        let mut history = History::default();
        let start = SystemTime::now();
        history.start_turn(question("What city is this?"), start);

        let started = history.start_turn(question("And now?"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
    }

    #[test]
    fn clear_forgets_every_turn() {
        let mut history = History::default();
        let start = SystemTime::now();
        ask_and_answer(&mut history, "What city is this?", start);
        history.clear();

        let started = history.start_turn(question("Hi"), start + Duration::from_secs(1));

        assert_eq!(started.messages.len(), 2);
        assert!(!started.forgot_earlier_turns);
    }
}
