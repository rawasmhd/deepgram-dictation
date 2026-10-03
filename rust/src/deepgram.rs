//! Deepgram: streaming over a WebSocket, and batch upload of a WAV file.
//! The query parameters are the same as in dictate.py.

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
    Api(u16),
    Network(String),
}

impl Error {
    /// The short text for the meter.
    pub fn message(&self) -> String {
        match self {
            Error::KeyRejected => "Key rejected - check your API key".into(),
            Error::Api(code) => format!("Deepgram error {code}"),
            Error::Network(_) => "No connection to Deepgram".into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::KeyRejected => write!(f, "Deepgram rejected the API key (401)"),
            Error::Api(code) => write!(f, "Deepgram returned HTTP {code}"),
            Error::Network(detail) => write!(f, "network: {detail}"),
        }
    }
}

/// Turn Deepgram's dictation newline tokens into real newlines.
pub fn format_transcript(text: &str) -> String {
    text.replace(r"<\n\n>", "\n\n").replace(r"<\n>", "\n")
}

fn query(extra: &[(&str, &str)]) -> String {
    PARAMS.iter().chain(extra).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")
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

pub fn transcribe(key: &str, samples: &[i16]) -> Result<String, Error> {
    let url = format!("https://{HOST}/v1/listen?{}", query(&[]));
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
            Err(Error::Api(code))
        }
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
    finals: Arc<Mutex<Vec<String>>>,
    done: Receiver<()>,
}

impl Stream {
    /// Connects in the background. Audio is buffered until the socket opens.
    pub fn start(key: String) -> (Stream, Sender<Vec<i16>>) {
        let (audio_tx, audio_rx) = mpsc::channel();
        let (done_tx, done) = mpsc::channel();
        let finals = Arc::new(Mutex::new(Vec::new()));
        let shared = finals.clone();
        thread::spawn(move || {
            if let Err(e) = run(&key, audio_rx, &shared) {
                log(&format!("streaming error: {e}"));
            }
            let _ = done_tx.send(());
        });
        (Stream { finals, done }, audio_tx)
    }

    /// Wait for the last results (the audio must have ended), then return
    /// all the final text. Empty if the connection failed.
    pub fn finish(self) -> String {
        let _ = self.done.recv_timeout(CONNECT_TIMEOUT + FINISH_TIMEOUT);
        let finals = self.finals.lock().unwrap();
        format_transcript(finals.join(" ").trim())
    }
}

type Socket = tungstenite::WebSocket<MaybeTlsStream<TcpStream>>;

fn run(key: &str, audio: Receiver<Vec<i16>>, finals: &Mutex<Vec<String>>) -> Result<(), String> {
    let mut socket = connect(key)?;
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
                        finals.lock().unwrap().push(final_text);
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

fn connect(key: &str) -> Result<Socket, String> {
    let url = format!("wss://{HOST}/v1/listen?{}", query(STREAM_PARAMS));
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    request
        .headers_mut()
        .insert("Authorization", format!("Token {key}").parse().map_err(|_| "bad API key")?);

    let addr = (HOST, 443).to_socket_addrs().map_err(|e| e.to_string())?.next().ok_or("no address")?;
    let tcp = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT).map_err(|e| e.to_string())?;
    tcp.set_nodelay(true).map_err(|e| e.to_string())?;
    let tls = tls().map_err(|e| format!("{e:?}"))?;
    let connector = tungstenite::Connector::NativeTls(tls);
    let (socket, _) = tungstenite::client_tls_with_config(request, tcp, None, Some(connector))
        .map_err(|e| match e {
            tungstenite::HandshakeError::Failure(tungstenite::Error::Http(r)) if r.status() == 401 => {
                "key rejected (401)".to_string()
            }
            other => other.to_string(),
        })?;

    // from here on, poll: send audio and read results in one loop
    match socket.get_ref() {
        MaybeTlsStream::NativeTls(s) => s.get_ref().set_nonblocking(true),
        MaybeTlsStream::Plain(s) => s.set_nonblocking(true),
        _ => Ok(()),
    }
    .map_err(|e| e.to_string())?;
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
    fn query_has_the_dictate_py_params() {
        let q = query(STREAM_PARAMS);
        assert!(q.starts_with("model=nova-3&language=en&"));
        assert!(q.ends_with("interim_results=false"));
    }
}
