//! Turns the Soniox token stream into chat-style utterances.
//!
//! Soniox sends each final token once and resends the whole non-final tail
//! with every response. Spoken (original) and translated tokens arrive in the
//! same stream, and an `<end>` token marks the moment a speaker paused, which
//! is where one chat bubble ends and the next begins.

use serde::Deserialize;

/// Marks the end of an utterance when endpoint detection is on.
const END_TOKEN: &str = "<end>";
/// Marks the point a manual `finalize` request reached. Carries no text.
const FINALIZE_TOKEN: &str = "<fin>";

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Token {
    pub text: String,
    #[serde(default)]
    pub is_final: bool,
    /// `"original"` or `"translation"` with translation on, `"none"` without.
    #[serde(default)]
    pub translation_status: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
}

impl Token {
    fn is_translation(&self) -> bool {
        self.translation_status.as_deref() == Some("translation")
    }
}

/// One chat bubble: what a speaker said between two pauses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Utterance {
    pub speaker: Option<String>,
    pub original: String,
    pub original_pending: String,
    pub translation: String,
    pub translation_pending: String,
}

impl Utterance {
    fn is_empty(&self) -> bool {
        self.original.is_empty()
            && self.original_pending.is_empty()
            && self.translation.is_empty()
            && self.translation_pending.is_empty()
    }
}

/// Which text a bubble shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    /// Polish translation, with the English below it.
    #[default]
    Both,
    Polish,
    English,
}

#[derive(Debug, Default)]
pub struct Transcript {
    finished: Vec<Utterance>,
    current: Utterance,
}

impl Transcript {
    /// Applies one Soniox response.
    pub fn apply(&mut self, tokens: &[Token]) {
        // Non-final tokens are resent in full with every response.
        self.current.original_pending.clear();
        self.current.translation_pending.clear();

        for token in tokens {
            match token.text.as_str() {
                "" | FINALIZE_TOKEN => continue,
                END_TOKEN => {
                    self.close_current();
                    continue;
                }
                _ => {}
            }
            if token.speaker.is_some() && !token.is_translation() {
                self.current.speaker.clone_from(&token.speaker);
            }
            let text = match (token.is_translation(), token.is_final) {
                (false, true) => &mut self.current.original,
                (false, false) => &mut self.current.original_pending,
                (true, true) => &mut self.current.translation,
                (true, false) => &mut self.current.translation_pending,
            };
            text.push_str(&token.text);
        }
    }

    /// Ends the current utterance, e.g. when the session stops.
    pub fn close_current(&mut self) {
        let mut current = std::mem::take(&mut self.current);
        // Anything still pending can no longer be revised.
        current
            .original
            .push_str(&std::mem::take(&mut current.original_pending));
        current
            .translation
            .push_str(&std::mem::take(&mut current.translation_pending));
        if !current.is_empty() {
            self.finished.push(current);
        }
    }

    pub fn utterances(&self) -> impl Iterator<Item = &Utterance> {
        self.finished
            .iter()
            .chain((!self.current.is_empty()).then_some(&self.current))
    }

    pub fn is_empty(&self) -> bool {
        self.utterances().next().is_none()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Plain text for the clipboard, in the same shape the window shows.
    pub fn to_text(&self, view: View) -> String {
        self.utterances()
            .map(|utterance| {
                let original = join(&utterance.original, &utterance.original_pending);
                let translation = join(&utterance.translation, &utterance.translation_pending);
                let body = match view {
                    View::English => original,
                    View::Polish => translation,
                    View::Both => format!("{translation}\n{original}"),
                };
                match &utterance.speaker {
                    Some(speaker) => format!("Speaker {speaker}:\n{body}"),
                    None => body,
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Final and pending text as one trimmed string.
pub fn join(done: &str, pending: &str) -> String {
    format!("{done}{pending}").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(text: &str, is_final: bool, status: &str, speaker: Option<&str>) -> Token {
        Token {
            text: text.into(),
            is_final,
            translation_status: Some(status.into()),
            speaker: speaker.map(Into::into),
        }
    }

    fn original(text: &str, is_final: bool) -> Token {
        token(text, is_final, "original", Some("1"))
    }

    fn translation(text: &str, is_final: bool) -> Token {
        token(text, is_final, "translation", None)
    }

    fn end() -> Token {
        Token {
            text: END_TOKEN.into(),
            is_final: true,
            ..Default::default()
        }
    }

    #[test]
    fn pending_text_is_replaced_by_each_response() {
        let mut transcript = Transcript::default();
        transcript.apply(&[original("Hel", false)]);
        transcript.apply(&[original("Hello", false), original(" every", false)]);
        let utterance = transcript.utterances().next().unwrap();
        assert_eq!(utterance.original, "");
        assert_eq!(utterance.original_pending, "Hello every");
    }

    #[test]
    fn final_tokens_accumulate() {
        let mut transcript = Transcript::default();
        transcript.apply(&[original("Hello", true), original(" every", false)]);
        transcript.apply(&[original(" everyone", true), translation("Cześć", false)]);
        let utterance = transcript.utterances().next().unwrap();
        assert_eq!(utterance.original, "Hello everyone");
        assert_eq!(utterance.original_pending, "");
        assert_eq!(utterance.translation_pending, "Cześć");
        assert_eq!(utterance.speaker.as_deref(), Some("1"));
    }

    #[test]
    fn end_token_starts_a_new_utterance() {
        let mut transcript = Transcript::default();
        transcript.apply(&[
            original("Hi", true),
            translation("Cześć", true),
            end(),
            token("Bye", false, "original", Some("2")),
        ]);
        let utterances: Vec<_> = transcript.utterances().collect();
        assert_eq!(utterances.len(), 2);
        assert_eq!(utterances[0].translation, "Cześć");
        assert_eq!(utterances[1].original_pending, "Bye");
        assert_eq!(utterances[1].speaker.as_deref(), Some("2"));
    }

    #[test]
    fn repeated_end_tokens_do_not_create_empty_bubbles() {
        let mut transcript = Transcript::default();
        transcript.apply(&[end(), original("Hi", true), end(), end()]);
        assert_eq!(transcript.utterances().count(), 1);
    }

    #[test]
    fn closing_keeps_pending_text() {
        let mut transcript = Transcript::default();
        transcript.apply(&[original("Hi", true), original(" there", false)]);
        transcript.close_current();
        transcript.apply(&[]);
        let utterances: Vec<_> = transcript.utterances().collect();
        assert_eq!(utterances.len(), 1);
        assert_eq!(utterances[0].original, "Hi there");
    }

    #[test]
    fn copies_text_in_the_chosen_view() {
        let mut transcript = Transcript::default();
        transcript.apply(&[original(" Hi", true), translation(" Cześć", true), end()]);
        assert_eq!(transcript.to_text(View::English), "Speaker 1:\nHi");
        assert_eq!(transcript.to_text(View::Polish), "Speaker 1:\nCześć");
        assert_eq!(transcript.to_text(View::Both), "Speaker 1:\nCześć\nHi");
    }

    #[test]
    fn parses_soniox_tokens() {
        let token: Token = serde_json::from_str(
            r#"{"text":"Hi","start_ms":10,"is_final":true,"speaker":"1","translation_status":"original"}"#,
        )
        .unwrap();
        assert_eq!(token, original("Hi", true));
    }
}
