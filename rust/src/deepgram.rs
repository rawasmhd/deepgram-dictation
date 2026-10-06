//! Deepgram: streaming over a WebSocket, and batch upload of a WAV file.

use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::Message;

use crate::audio::SAMPLE_RATE;
use crate::log;
use crate::words;

const HOST: &str = "api.deepgram.com";

// docs: developers.deepgram.com/docs/pre-recorded-audio
const PARAMS: &[(&str, &str)] = &[
    ("model", "nova-3"),
    ("language", "en"),
    ("smart_format", "true"),
    ("punctuate", "true"),
    ("dictation", "true"), // spoken "comma" -> ","
    ("filler_words", "false"),
];

// what the streaming endpoint needs to read the raw PCM we send
const STREAM_PARAMS: &[(&str, &str)] = &[
    ("encoding", "linear16"),
    ("sample_rate", "16000"),
    ("channels", "1"),
    ("interim_results", "false"), // only final text, like batch
];

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const BATCH_TIMEOUT: Duration = Duration::from_secs(60);
/// How long to wait for the last results after the audio ends.
const FINISH_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub enum Error {
    KeyRejected,
    /// The custom words list is too long (more than 500 tokens).
    KeytermsRejected,
    Api(u16),
    Network(String),
}

impl Error {
    /// The short text for the meter.
    pub fn message(&self) -> String {
        match self {
            Error::KeyRejected => "Key rejected - check your API key".into(),
            Error::KeytermsRejected => "Custom words list too long".into(),
            Error::Api(code) => format!("Deepgram error {code}"),
            Error::Network(_) => "No connection to Deepgram".into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::KeyRejected => write!(f, "Deepgram rejected the API key (401)"),
            Error::KeytermsRejected => write!(f, "Deepgram rejected the custom words (keyterm limit)"),
            Error::Api(code) => write!(f, "Deepgram returned HTTP {code}"),
            Error::Network(detail) => write!(f, "network: {detail}"),
        }
    }
}

/// Turn Deepgram's dictation newline tokens into real newlines.
pub fn format_transcript(text: &str) -> String {
    text.replace(r"<\n\n>", "\n\n").replace(r"<\n>", "\n")
}

/// The query string: our settings, then one keyterm parameter per term.
fn query(extra: &[(&str, &str)], terms: &[String]) -> String {
    let params = PARAMS.iter().chain(extra).map(|(k, v)| format!("{k}={v}"));
    let terms = terms.iter().map(|t| format!("keyterm={}", words::url_encode(t)));
    params.chain(terms).collect::<Vec<_>>().join("&")
}

/// A 400 that names the key terms: the list is too long.
fn keyterm_error(code: u16, body: &str) -> bool {
    code == 400 && body.to_ascii_lowercase().contains("keyterm")
}

fn tls() -> Result<native_tls::TlsConnector, Error> {
    native_tls::TlsConnector::new().map_err(|e| Error::Network(e.to_string()))
}

// -- batch --------------------------------------------------------------------

/// Reused across calls, so later uploads skip the TLS handshake.
fn agent() -> Result<&'static ureq::Agent, Error> {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    if let Some(a) = AGENT.get() {
        return Ok(a);
    }
    let agent = ureq::AgentBuilder::new().tls_connector(Arc::new(tls()?)).timeout(BATCH_TIMEOUT).build();
    Ok(AGENT.get_or_init(|| agent))
}

pub fn transcribe(key: &str, samples: &[i16], terms: &[String]) -> Result<String, Error> {
    let url = format!("https://{HOST}/v1/listen?{}", query(&[], terms));
    let response = agent()?
        .post(&url)
        .set("Authorization", &format!("Token {key}"))
        .set("Content-Type", "audio/wav")
        .send_bytes(&wav(samples));
    match response {
        Ok(r) => {
            let body: serde_json::Value =
                serde_json::from_reader(r.into_reader()).map_err(|e| Error::Network(e.to_string()))?;
            let text = body["results"]["channels"][0]["alternatives"][0]["transcript"].as_str().unwrap_or("");
            Ok(format_transcript(text.trim()))
        }
        Err(ureq::Error::Status(401, _)) => Err(Error::KeyRejected),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            log(&format!("Deepgram {code}: {}", body.chars().take(200).collect::<String>()));
            if keyterm_error(code, &body) {
                return Err(Error::KeytermsRejected);
            }
            Err(Error::Api(code))
        }
        Err(ureq::Error::Transport(t)) => Err(Error::Network(t.to_string())),
    }
}

/// Check a key before it is saved: Deepgram answers 401 to a wrong key.
pub fn check_key(key: &str) -> Result<(), Error> {
    match agent()?.get(&format!("https://{HOST}/v1/projects")).set("Authorization", &format!("Token {key}")).call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(401, _)) => Err(Error::KeyRejected),
        // a key without permission to list projects can still transcribe
        Err(ureq::Error::Status(_, _)) => Ok(()),
        Err(ureq::Error::Transport(t)) => Err(Error::Network(t.to_string())),
    }
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

// -- streaming ----------------------------------------------------------------

/// A live WebSocket session. Audio sent to `sender()` is transcribed as it
/// arrives. When every sender is dropped, the session asks Deepgram to
/// finish, and `finish()` returns the text.
pub struct Stream {
    transcript: Transcript,
    done: Receiver<()>,
}

/// The final text so far, readable while the session runs.
#[derive(Clone, Default)]
pub struct Transcript(Arc<Mutex<Vec<String>>>);

impl Transcript {
    pub fn text(&self) -> String {
        format_transcript(self.0.lock().unwrap().join(" ").trim())
    }
}

impl Stream {
    /// Connects in the background. Audio is buffered until the socket opens.
    /// `on_final` runs on the network thread after each final phrase.
    pub fn start(
        key: String,
        terms: Vec<String>,
        on_final: impl Fn() + Send + 'static,
    ) -> (Stream, Sender<Vec<i16>>) {
        let (audio_tx, audio_rx) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        let transcript = Transcript::default();
        let shared = transcript.clone();
        thread::spawn(move || {
            if let Err(e) = run(&key, &terms, audio_rx, &shared, &on_final) {
                log(&format!("streaming error: {e}"));
            }
            let _ = done_tx.send(());
        });
        (Stream { transcript, done }, audio_tx)
    }

    pub fn transcript(&self) -> Transcript {
        self.transcript.clone()
    }

    /// Wait for the last results (the audio must have ended), then return
    /// all the final text. Empty if the connection failed.
    pub fn finish(self) -> String {
        let _ = self.done.recv_timeout(CONNECT_TIMEOUT + FINISH_TIMEOUT);
        self.transcript.text()
    }
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

fn run(
    key: &str,
    terms: &[String],
    audio: Receiver<Vec<i16>>,
    transcript: &Transcript,
    on_final: &dyn Fn(),
) -> Result<(), String> {
    let mut socket = match connect(key, terms) {
        // the audio waits in the channel, so nothing is lost
        Err(Error::KeytermsRejected) => {
            words::reject(terms);
            connect(key, &[])
        }
        other => other,
    }
    .map_err(|e| e.to_string())?;
    let mut closing: Option<Instant> = None;

    loop {
        // audio out
        if closing.is_none() {
            loop {
                match audio.try_recv() {
                    Ok(block) => {
                        let bytes = block.iter().flat_map(|s| s.to_le_bytes()).collect();
                        write(&mut socket, Message::Binary(bytes))?;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        // the recording ended: ask for the last results
                        write(&mut socket, Message::Text(r#"{"type":"CloseStream"}"#.into()))?;
                        closing = Some(Instant::now());
                        break;
                    }
                }
            }
        }
        match socket.flush() {
            Ok(()) => {}
            Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => return Err(e.to_string()),
        }

        // results in
        loop {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    if let Some(final_text) = final_transcript(&text) {
                        transcript.0.lock().unwrap().push(final_text);
                        on_final();
                    }
                }
                Ok(Message::Close(_)) => return Ok(()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => break,
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    return Ok(())
                }
                Err(e) if closing.is_some() => {
                    // the server's close can surface as an error; that is normal
                    log(&format!("streaming closed: {e}"));
                    return Ok(());
                }
                Err(e) => return Err(e.to_string()),
            }
        }

        if closing.is_some_and(|t| t.elapsed() > FINISH_TIMEOUT) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn connect(key: &str, terms: &[String]) -> Result<Socket, Error> {
    let net = |e: &dyn std::fmt::Display| Error::Network(e.to_string());
    let url = format!("wss://{HOST}/v1/listen?{}", query(STREAM_PARAMS, terms));
    let mut request = url.into_client_request().map_err(|e| net(&e))?;
    request
        .headers_mut()
        .insert("Authorization", format!("Token {key}").parse().map_err(|_| net(&"bad API key"))?);

    let addr = (HOST, 443).to_socket_addrs().map_err(|e| net(&e))?.next().ok_or_else(|| net(&"no address"))?;
    let tcp = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| net(&e))?;
    tcp.set_nodelay(true).map_err(|e| net(&e))?;
    let connector = tungstenite::Connector::NativeTls(tls()?);
    let (socket, _) = tungstenite::client_tls_with_config(request, tcp, None, Some(connector))
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(tungstenite::Error::Http(r)) => {
                let code = r.status().as_u16();
                let header = r.headers().get("dg-error").and_then(|v| v.to_str().ok()).unwrap_or("");
                let body = r.body().as_deref().map(String::from_utf8_lossy).unwrap_or_default();
                log(&format!("Deepgram {code}: {header} {}", body.chars().take(200).collect::<String>()));
                match code {
                    401 => Error::KeyRejected,
                    _ if keyterm_error(code, &format!("{header} {body}")) => Error::KeytermsRejected,
                    _ => Error::Api(code),
                }
            }
            other => net(&other),
        })?;

    // from here on, poll: send audio and read results in one loop
    match socket.get_ref() {
        MaybeTlsStream::NativeTls(s) => s.get_ref().set_nonblocking(true),
        MaybeTlsStream::Plain(s) => s.set_nonblocking(true),
        _ => Ok(()),
    }
    .map_err(|e| net(&e))?;
    Ok(socket)
}

fn write(socket: &mut Socket, message: Message) -> Result<(), String> {
    match socket.write(message) {
        Ok(()) => Ok(()),
        // queued; the next flush sends it
        Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The transcript of a final result, if the message is one and has text.
fn final_transcript(message: &str) -> Option<String> {
    let data: serde_json::Value = serde_json::from_str(message).ok()?;
    if !data["is_final"].as_bool().unwrap_or(false) {
        return None;
    }
    let text = data["channel"]["alternatives"][0]["transcript"].as_str()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newline_tokens_become_newlines() {
        assert_eq!(format_transcript(r"one<\n>two<\n\n>three"), "one\ntwo\n\nthree");
    }

    #[test]
    fn only_final_results_with_text_count() {
        let msg = |text: &str, fin: bool| {
            serde_json::json!({"is_final": fin, "channel": {"alternatives": [{"transcript": text}]}})
                .to_string()
        };
        assert_eq!(final_transcript(&msg("hello", true)), Some("hello".into()));
        assert_eq!(final_transcript(&msg("hello", false)), None);
        assert_eq!(final_transcript(&msg("  ", true)), None);
        assert_eq!(final_transcript("not json"), None);
        assert_eq!(final_transcript(r#"{"type":"Metadata"}"#), None);
    }

    #[test]
    fn wav_header_is_valid() {
        let bytes = wav(&[0, 1, -1]);
        let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!((spec.sample_rate, spec.channels, spec.bits_per_sample), (16_000, 1, 16));
        let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
        assert_eq!(samples, [0, 1, -1]);
    }

    #[test]
    fn query_has_the_expected_params() {
        let q = query(STREAM_PARAMS, &[]);
        assert!(q.starts_with("model=nova-3&language=en&"));
        assert!(q.ends_with("interim_results=false"));
    }

    #[test]
    fn query_has_one_keyterm_per_term() {
        let terms = ["Rawas".to_string(), "GitHub Actions".to_string()];
        let q = query(&[], &terms);
        assert!(q.ends_with("filler_words=false&keyterm=Rawas&keyterm=GitHub%20Actions"));
        assert!(!query(&[], &[]).contains("keyterm"));
    }

    #[test]
    fn keyterm_limit_error_is_recognized() {
        // the text Deepgram returned in a test on 2026-10-06
        let body = r#"{"err_code":"Bad Request","err_msg":"Bad Request: Keyterm limit exceeded. The maximum number of tokens across all keyterms is 500."}"#;
        assert!(keyterm_error(400, body));
        assert!(!keyterm_error(400, r#"{"err_msg":"Bad Request: unknown model"}"#));
        assert!(!keyterm_error(500, body));
    }

    /// Calls Deepgram (costs a little). Run with a key:
    /// DEEPGRAM_API_KEY=... cargo test -- --ignored live_keyterms
    #[test]
    #[ignore]
    fn live_keyterms() {
        let key = std::env::var("DEEPGRAM_API_KEY").expect("set DEEPGRAM_API_KEY");
        let silence = vec![0i16; 16_000];
        let few = vec!["Rawas".to_string(), "GitHub Actions".to_string()];
        let many: Vec<String> = (0..300).map(|i| format!("Zyxquar Plimbet Vorshnik{i}")).collect();
        assert!(transcribe(&key, &silence, &few).is_ok());
        assert!(matches!(transcribe(&key, &silence, &many), Err(Error::KeytermsRejected)));
        assert!(connect(&key, &few).is_ok());
        assert!(matches!(connect(&key, &many), Err(Error::KeytermsRejected)));
    }
}
