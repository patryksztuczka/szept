//! Soniox real-time speech-to-text over a websocket. One session streams
//! 16 kHz mono PCM in and gets English tokens plus their Polish translation
//! back. See https://soniox.com/docs/stt/rt/real-time-translation.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest, http::header};

use crate::transcript::Token;

const URL: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const MODEL: &str = "stt-rt-v5";
pub const SAMPLE_RATE: u32 = 16_000;
/// How long to wait for the last tokens after the audio ends.
const FINISH_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug)]
pub enum Event {
    Tokens(Vec<Token>),
    /// Peak level of the latest audio chunk, 0.0 to 1.0.
    Level(f32),
    /// Something worth showing that does not end the session.
    Notice(String),
    Failed(String),
    /// The session is over; no more events follow.
    Finished,
}

/// The first message of every session.
fn config() -> serde_json::Value {
    json!({
        "model": MODEL,
        "audio_format": "pcm_s16le",
        "sample_rate": SAMPLE_RATE,
        "num_channels": 1,
        "language_hints": ["en"],
        "enable_speaker_diarization": true,
        "enable_endpoint_detection": true,
        // Always translate, so switching views never restarts the session.
        "translation": { "type": "one_way", "target_language": "pl" },
    })
}

#[derive(Debug, Deserialize)]
struct Response {
    #[serde(default)]
    tokens: Vec<Token>,
    #[serde(default)]
    finished: bool,
    #[serde(default)]
    error_code: Option<serde_json::Value>,
    #[serde(default)]
    error_message: Option<String>,
}

impl Response {
    fn error(&self) -> Option<String> {
        let code = self.error_code.as_ref()?;
        let message = self.error_message.as_deref().unwrap_or("unknown error");
        Some(match code.as_u64() {
            Some(401) => "Soniox rejected the API key. Check it in Settings.".to_owned(),
            Some(402) => "Your Soniox account is out of credit.".to_owned(),
            _ => format!("Soniox error {code}: {message}"),
        })
    }
}

/// Runs one session on the calling thread until the audio channel closes and
/// Soniox sends its last tokens. Always ends with [`Event::Finished`].
pub fn run(
    api_key: &str,
    audio: UnboundedReceiver<Vec<u8>>,
    events: &async_channel::Sender<Event>,
) {
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
        .and_then(|runtime| runtime.block_on(session(api_key, audio, events)));
    if let Err(error) = result {
        tracing::warn!(%error, "soniox session failed");
        events.send_blocking(Event::Failed(error)).ok();
    }
    events.send_blocking(Event::Finished).ok();
}

async fn session(
    api_key: &str,
    mut audio: UnboundedReceiver<Vec<u8>>,
    events: &async_channel::Sender<Event>,
) -> Result<(), String> {
    let mut request = URL.into_client_request().map_err(|e| e.to_string())?;
    let authorization = format!("Bearer {api_key}")
        .parse()
        .map_err(|_| "The API key contains characters that are not allowed.".to_owned())?;
    request
        .headers_mut()
        .insert(header::AUTHORIZATION, authorization);

    let (socket, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|error| format!("Could not connect to Soniox: {error}"))?;
    tracing::info!("connected to soniox");
    let (mut sink, mut stream) = socket.split();
    sink.send(Message::Text(config().to_string().into()))
        .await
        .map_err(|e| e.to_string())?;

    let mut audio_open = true;
    let finish_deadline = tokio::time::sleep(Duration::MAX);
    tokio::pin!(finish_deadline);
    loop {
        tokio::select! {
            chunk = audio.recv(), if audio_open => match chunk {
                Some(bytes) => sink
                    .send(Message::Binary(bytes.into()))
                    .await
                    .map_err(|e| e.to_string())?,
                None => {
                    // An empty text message tells Soniox the audio has ended.
                    audio_open = false;
                    sink.send(Message::Text("".into())).await.map_err(|e| e.to_string())?;
                    finish_deadline.as_mut().reset(tokio::time::Instant::now() + FINISH_TIMEOUT);
                }
            },
            message = stream.next() => match message {
                None => return Ok(()),
                Some(Err(error)) => return Err(format!("Lost the connection to Soniox: {error}")),
                Some(Ok(Message::Text(text))) => {
                    let response: Response =
                        serde_json::from_str(&text).map_err(|e| e.to_string())?;
                    if let Some(error) = response.error() {
                        return Err(error);
                    }
                    if !response.tokens.is_empty()
                        && events.send(Event::Tokens(response.tokens)).await.is_err()
                    {
                        return Ok(());
                    }
                    if response.finished {
                        sink.close().await.ok();
                        return Ok(());
                    }
                }
                Some(Ok(Message::Close(frame))) => {
                    return match frame {
                        Some(frame) if !frame.reason.is_empty() => {
                            Err(format!("Soniox closed the session: {}", frame.reason))
                        }
                        _ => Ok(()),
                    };
                }
                Some(Ok(_)) => {}
            },
            () = &mut finish_deadline => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_requests_polish_translation_of_raw_pcm() {
        let config = config();
        assert_eq!(config["audio_format"], "pcm_s16le");
        assert_eq!(config["sample_rate"], SAMPLE_RATE);
        assert_eq!(config["translation"]["target_language"], "pl");
    }

    #[test]
    fn parses_tokens_and_errors() {
        let response: Response = serde_json::from_str(
            r#"{"tokens":[{"text":"Hi","is_final":true}],"final_audio_proc_ms":0,"total_audio_proc_ms":120}"#,
        )
        .unwrap();
        assert_eq!(response.tokens.len(), 1);
        assert!(response.error().is_none());

        let response: Response =
            serde_json::from_str(r#"{"tokens":[],"error_code":401,"error_message":"Invalid"}"#)
                .unwrap();
        assert!(response.error().unwrap().contains("API key"));
    }
}
