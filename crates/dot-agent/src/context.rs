//! What the model is sent for one question: the system prompt, earlier turns as text, the new question with its screenshot.
//! Must not decide which turns are remembered or when they are forgotten; history.rs does.

use dot_providers::{ChatMessage, ChatRole};

/// A typed or spoken question and the screenshot taken when it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserInput {
    pub text: String,
    pub jpeg_screenshot: Option<Vec<u8>>,
}

pub(crate) const SYSTEM_PROMPT: &str = "You are Hey Dot, an assistant on the user's Mac. When a screenshot is attached, it shows the user's screen at the moment they asked; use it to answer. Answer briefly and directly.";

const EARLIER_SCREENSHOT_NOTE: &str = "[screenshot from earlier turn]";

/// A question already sent. Its screenshot is never kept, only whether it had one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Turn {
    pub(crate) question: String,
    pub(crate) had_screenshot: bool,
    pub(crate) answer: String,
}

pub(crate) fn build_messages(earlier: &[Turn], input: UserInput) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage {
        role: ChatRole::System,
        text: SYSTEM_PROMPT.to_owned(),
        jpeg_image: None,
    }];
    for turn in earlier {
        // The note follows the question because a request puts a message's text before its image.
        let question = if turn.had_screenshot {
            format!("{}\n{EARLIER_SCREENSHOT_NOTE}", turn.question)
        } else {
            turn.question.clone()
        };
        messages.push(ChatMessage {
            role: ChatRole::User,
            text: question,
            jpeg_image: None,
        });
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            text: turn.answer.clone(),
            jpeg_image: None,
        });
    }
    messages.push(ChatMessage {
        role: ChatRole::User,
        text: input.text,
        jpeg_image: input.jpeg_screenshot,
    });
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earlier_turn(question: &str, had_screenshot: bool, answer: &str) -> Turn {
        Turn {
            question: question.to_owned(),
            had_screenshot,
            answer: answer.to_owned(),
        }
    }

    fn with_screenshot(text: &str) -> UserInput {
        UserInput {
            text: text.to_owned(),
            jpeg_screenshot: Some(vec![0xFF, 0xD8]),
        }
    }

    #[test]
    fn the_system_prompt_comes_first() {
        let messages = build_messages(&[], with_screenshot("What is on my screen?"));

        assert_eq!(messages[0].role, ChatRole::System);
        assert_eq!(messages[0].text, SYSTEM_PROMPT);
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn only_the_new_question_carries_its_screenshot() {
        let earlier = [earlier_turn("What city is this?", true, "Paris")];

        let messages = build_messages(&earlier, with_screenshot("How many people live there?"));

        let roles: Vec<ChatRole> = messages.iter().map(|message| message.role).collect();
        assert_eq!(
            roles,
            [
                ChatRole::System,
                ChatRole::User,
                ChatRole::Assistant,
                ChatRole::User
            ]
        );
        assert_eq!(messages[1].jpeg_image, None);
        assert_eq!(messages[2].text, "Paris");
        assert_eq!(messages[3].text, "How many people live there?");
        assert_eq!(messages[3].jpeg_image, Some(vec![0xFF, 0xD8]));
    }

    #[test]
    fn an_earlier_screenshot_leaves_a_note_after_its_question() {
        let earlier = [earlier_turn("What city is this?", true, "Paris")];

        let messages = build_messages(&earlier, with_screenshot("And its population?"));

        assert_eq!(
            messages[1].text,
            "What city is this?\n[screenshot from earlier turn]"
        );
    }

    #[test]
    fn an_earlier_question_without_a_screenshot_has_no_note() {
        let earlier = [earlier_turn(
            "What is the capital of France?",
            false,
            "Paris",
        )];

        let messages = build_messages(&earlier, with_screenshot("And of Spain?"));

        assert_eq!(messages[1].text, "What is the capital of France?");
    }
}
