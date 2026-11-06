//! Speaking instead of typing (LC-475): the microphone button of the message field records speech and whisper.cpp
//! (compiled in, CPU only) turns it into text in the language of the interface or the one chosen for speech. The text
//! goes into the message field to be corrected; nothing is sent by itself.
//!
//! Nothing of the recording leaves the computer. Only the speech model is downloaded, once, after the user agrees
//! (Hugging Face, a fixed revision, checked by SHA-256) – never when the computer's policy switches web access off
//! (WebAccess=0); IT can then place the model file in `%ProgramData%\Loonar Compass\speech` instead. The computer's
//! policy Dictation=0 removes the microphone button.

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

/// A speech model of whisper.cpp (multilingual, quantised q5_1).
pub struct SpeechModel {
    /// The value of the setting `dictation.model`.
    pub name: &'static str,
    pub file: &'static str,
    sha256: &'static str,
    pub megabytes: u32,
}

/// "small" recognises Polish, German and French well; "base" is about four times faster and makes more mistakes.
pub const MODELS: [SpeechModel; 2] = [
    SpeechModel { name: "small", file: "ggml-small-q5_1.bin", sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb", megabytes: 190 },
    SpeechModel { name: "base", file: "ggml-base-q5_1.bin", sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898", megabytes: 60 },
];

/// The models of the whisper.cpp project at a fixed revision (the file is also checked by its SHA-256).
const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/";

const SAMPLE_RATE: usize = 16_000;
/// The longest recording; it then ends by itself and is recognised.
pub const MAX_SECONDS: u64 = 120;

/// The model of the setting (unknown = "small").
pub fn model(name: &str) -> &'static SpeechModel {
    MODELS.iter().find(|m| m.name == name).unwrap_or(&MODELS[0])
}

/// The folder where IT can place the models for every user of the computer.
pub fn shared_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into())).join(crate::config::APP_NAME).join("speech")
}

/// The folder of the models downloaded by the user.
fn user_dir() -> PathBuf {
    crate::config::user_data_dir().join("speech")
}

/// The model file on this computer: IT's folder first, then the user's.
pub fn model_path(m: &SpeechModel) -> Option<PathBuf> {
    [shared_dir(), user_dir()].into_iter().map(|d| d.join(m.file)).find(|p| p.is_file())
}

/// What a download of `m` brings: `m`, and the faster "base" for the text shown while speaking (LC-485), each only
/// when it is not on this computer yet.
pub fn download_set(m: &'static SpeechModel) -> Vec<&'static SpeechModel> {
    let base = model("base");
    let mut set: Vec<&'static SpeechModel> = Vec::new();
    if model_path(m).is_none() {
        set.push(m);
    }
    if m.name != base.name && model_path(base).is_none() {
        set.push(base);
    }
    set
}

/// The megabytes of `download_set`.
pub fn download_megabytes(m: &'static SpeechModel) -> u32 {
    download_set(m).iter().map(|m| m.megabytes).sum()
}

/// The recogniser is built for AVX2, FMA and F16C (every PC since about 2015); without them it is not used at all.
pub fn cpu_supported() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") && is_x86_feature_detected!("f16c")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}

/// The language code of speech: the setting, or the language of the interface when it is empty.
pub fn language(setting: &str) -> &'static str {
    if setting.trim().is_empty() {
        crate::i18n::current().code()
    } else {
        crate::i18n::Lang::parse(setting).code()
    }
}

/// What a running dictation is doing.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    /// Downloading the model: bytes done and the total.
    Downloading(u64, u64),
    Recording,
    Recognising,
}

/// The end of a dictation.
pub enum Outcome {
    /// The recognised text (may be empty when nothing was said).
    Text(String),
    /// The model was downloaded; the next click records.
    Downloaded,
    /// Stopped by the user before anything came out (a download or a recognition).
    Cancelled,
    Failed(String),
}

enum Event {
    Phase(Phase),
    /// What was recognised so far, while still speaking (LC-485).
    Partial(String),
    Done(Outcome),
}

/// A dictation in a worker thread: a model download, or a recording followed by its recognition.
pub struct Dictation {
    stop: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    level: Arc<AtomicU32>,
    rx: mpsc::Receiver<Event>,
    pub phase: Phase,
    pub started: Instant,
    /// The text recognised so far, shown while speaking (LC-485); the final text replaces it.
    pub partial: String,
}

impl Dictation {
    /// Downloads the model into the user's folder.
    pub fn download(m: &'static SpeechModel, wake: impl Fn() + Send + 'static) -> Self {
        let set = download_set(m);
        let total = u64::from(set.iter().map(|m| m.megabytes).sum::<u32>()) << 20;
        let (dictation, tx, _stop, cancel, _level) = Self::new(Phase::Downloading(0, total));
        std::thread::spawn(move || {
            // One after another, with one progress for all of them.
            let mut before = 0u64;
            let mut result = Ok(PathBuf::new());
            for m in set {
                let progress = |done: u64, _: u64| {
                    let _ = tx.send(Event::Phase(Phase::Downloading(before + done, total)));
                    wake();
                };
                result = download(m, &cancel, progress);
                if result.is_err() {
                    break;
                }
                before += u64::from(m.megabytes) << 20;
            }
            let outcome = match result {
                Ok(_) => Outcome::Downloaded,
                Err(_) if cancel.load(Ordering::Relaxed) => Outcome::Cancelled,
                Err(e) => Outcome::Failed(crate::i18n::fmt("Could not download the speech model: {0}", &[format!("{e:#}")])),
            };
            let _ = tx.send(Event::Done(outcome));
            wake();
        });
        dictation
    }

    /// Records from the default microphone until `finish` (or `MAX_SECONDS`), then recognises the speech. While it
    /// records, what was said so far is recognised about every second and sent as `partial` (LC-485: the text came
    /// only at the end); the whole recording is recognised once more at the end.
    pub fn record(model_file: PathBuf, language: &'static str, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (dictation, tx, stop, cancel, level) = Self::new(Phase::Recording);
        std::thread::spawn(move || {
            let wake = Arc::new(wake);
            let ticker = Arc::clone(&wake);
            let heard = Arc::new(std::sync::Mutex::new(Vec::<f32>::new()));
            // The text while speaking comes from the faster "base" when it is here; the chosen model, loaded meanwhile,
            // recognises the whole recording at the end.
            let live_file = model_path(model("base")).unwrap_or_else(|| model_file.clone());
            let preload = (live_file != model_file).then(|| {
                let model_file = model_file.clone();
                std::thread::spawn(move || load(&model_file).ok())
            });
            let live = {
                let (heard, stop, tx, wake) = (Arc::clone(&heard), Arc::clone(&stop), tx.clone(), Arc::clone(&wake));
                std::thread::spawn(move || live_text(&live_file, language, &heard, &stop, |text| {
                    let _ = tx.send(Event::Partial(text));
                    wake();
                }))
            };
            let outcome = (|| -> Result<Outcome> {
                let recorded = record(&stop, &level, &heard, || ticker());
                // The live recognition ends with the recording (stop is set) and hands over the loaded model.
                let live_context = live.join().ok().flatten();
                let context = match preload {
                    Some(loading) => loading.join().ok().flatten(),
                    None => live_context,
                };
                let pcm = recorded?;
                if cancel.load(Ordering::Relaxed) {
                    return Ok(Outcome::Cancelled);
                }
                let _ = tx.send(Event::Phase(Phase::Recognising));
                wake();
                let text = match &context {
                    Some(context) => recognise_with(context, language, &pcm, &cancel),
                    None => recognise(&model_file, language, &pcm, &cancel),
                };
                if cancel.load(Ordering::Relaxed) {
                    return Ok(Outcome::Cancelled);
                }
                Ok(Outcome::Text(text?))
            })()
            .unwrap_or_else(|e| Outcome::Failed(format!("{e:#}")));
            let _ = tx.send(Event::Done(outcome));
            wake();
        });
        dictation
    }

    #[allow(clippy::type_complexity)]
    fn new(phase: Phase) -> (Self, mpsc::Sender<Event>, Arc<AtomicBool>, Arc<AtomicBool>, Arc<AtomicU32>) {
        let (tx, rx) = mpsc::channel();
        let (stop, cancel, level) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicU32::new(0)));
        let dictation = Self { stop: Arc::clone(&stop), cancel: Arc::clone(&cancel), level: Arc::clone(&level), rx, phase, started: Instant::now(), partial: String::new() };
        (dictation, tx, stop, cancel, level)
    }

    /// Ends the recording; the speech is then recognised.
    pub fn finish(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Stops everything: the recording is thrown away, a download or a recognition is broken off.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.stop.store(true, Ordering::Relaxed);
    }

    /// The loudness of the last moment of the recording, 0..1 (for the level shown at the button).
    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    /// The news of the worker: the phase is updated; `Some` once the dictation is over.
    pub fn poll(&mut self) -> Option<Outcome> {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Phase(phase) => self.phase = phase,
                Event::Partial(text) => self.partial = text,
                Event::Done(outcome) => return Some(outcome),
            }
        }
        None
    }
}

/// Downloads the model to `<user data>\speech` (through a `.part` file, checked by SHA-256).
fn download(m: &SpeechModel, cancel: &AtomicBool, progress: impl Fn(u64, u64)) -> Result<PathBuf> {
    download_to(m, user_dir(), cancel, progress)
}

fn download_to(m: &SpeechModel, dir: PathBuf, cancel: &AtomicBool, progress: impl Fn(u64, u64)) -> Result<PathBuf> {
    std::fs::create_dir_all(&dir).with_context(|| dir.display().to_string())?;
    let target = dir.join(m.file);
    let part = dir.join(format!("{}.part", m.file));
    let client = reqwest::blocking::Client::builder().connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(3600)).build()?;
    let mut response = client.get(format!("{MODEL_BASE_URL}{}", m.file)).send()?.error_for_status()?;
    let total = response.content_length().unwrap_or(u64::from(m.megabytes) << 20);
    let mut file = std::fs::File::create(&part).with_context(|| part.display().to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    let (mut done, mut shown) = (0u64, Instant::now());
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            bail!("cancelled");
        }
        let n = response.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        hasher.update(&buffer[..n]);
        done += n as u64;
        if shown.elapsed() > Duration::from_millis(200) {
            progress(done, total);
            shown = Instant::now();
        }
    }
    file.flush()?;
    drop(file);
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if digest != m.sha256 {
        let _ = std::fs::remove_file(&part);
        bail!("{}", crate::i18n::tr("the downloaded file is not the expected model (SHA-256 differs)"));
    }
    std::fs::rename(&part, &target)?;
    Ok(target)
}

/// 16 kHz mono samples from the default microphone (Windows waveIn, which converts from the device's format).
#[cfg(windows)]
fn record(stop: &AtomicBool, level: &AtomicU32, heard: &std::sync::Mutex<Vec<f32>>, tick: impl Fn()) -> Result<Vec<f32>> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::Media::{MMSYSERR_ALLOCATED, MMSYSERR_NOERROR};

    const CHUNK: usize = SAMPLE_RATE / 10;
    const CHUNKS: usize = 8;
    if unsafe { waveInGetNumDevs() } == 0 {
        bail!("{}", crate::i18n::tr("No microphone was found. Connect one, or switch it on in the Windows sound settings."));
    }
    let format = WAVEFORMATEX { wFormatTag: WAVE_FORMAT_PCM as u16, nChannels: 1, nSamplesPerSec: SAMPLE_RATE as u32, nAvgBytesPerSec: SAMPLE_RATE as u32 * 2, nBlockAlign: 2, wBitsPerSample: 16, cbSize: 0 };
    let mut device = HWAVEIN::default();
    let rc = unsafe { waveInOpen(Some(&mut device), WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL) };
    if rc != MMSYSERR_NOERROR {
        if rc == MMSYSERR_ALLOCATED {
            bail!("{}", crate::i18n::tr("The microphone is used by another application."));
        }
        bail!("{}", crate::i18n::fmt("The microphone could not be opened (error {0}).", &[rc.to_string()]));
    }
    let header_size = std::mem::size_of::<WAVEHDR>() as u32;
    // The buffers and their headers stay at the same address until the device is closed.
    let mut buffers: Vec<Box<[i16; CHUNK]>> = (0..CHUNKS).map(|_| Box::new([0i16; CHUNK])).collect();
    let mut headers: Vec<Box<WAVEHDR>> = buffers
        .iter_mut()
        .map(|b| Box::new(WAVEHDR { lpData: windows::core::PSTR(b.as_mut_ptr() as *mut u8), dwBufferLength: (CHUNK * 2) as u32, ..Default::default() }))
        .collect();
    let mut pcm: Vec<f32> = Vec::with_capacity(SAMPLE_RATE * 30);
    let mut peak = 0f32;
    let mut take = |header: &WAVEHDR, buffer: &[i16; CHUNK], pcm: &mut Vec<f32>| {
        let samples = &buffer[..(header.dwBytesRecorded as usize / 2).min(CHUNK)];
        let mut loudest = 0f32;
        let from = pcm.len();
        for s in samples {
            let v = f32::from(*s) / 32768.0;
            loudest = loudest.max(v.abs());
            pcm.push(v);
        }
        // For the live recognition (LC-485).
        if let Ok(mut shared) = heard.lock() {
            shared.extend_from_slice(&pcm[from..]);
        }
        peak = peak.max(loudest);
        level.store(loudest.to_bits(), Ordering::Relaxed);
    };
    let result = (|| -> Result<()> {
        for h in headers.iter_mut() {
            let h: *mut WAVEHDR = &mut **h;
            if unsafe { waveInPrepareHeader(device, h, header_size) } != MMSYSERR_NOERROR || unsafe { waveInAddBuffer(device, h, header_size) } != MMSYSERR_NOERROR {
                bail!("{}", crate::i18n::tr("The microphone could not be started."));
            }
        }
        if unsafe { waveInStart(device) } != MMSYSERR_NOERROR {
            bail!("{}", crate::i18n::tr("The microphone could not be started."));
        }
        let started = Instant::now();
        let mut next = 0;
        while !stop.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(MAX_SECONDS) {
            while headers[next].dwFlags & WHDR_DONE != 0 {
                take(&headers[next], &buffers[next], &mut pcm);
                headers[next].dwFlags &= !WHDR_DONE;
                let h: *mut WAVEHDR = &mut *headers[next];
                unsafe { waveInAddBuffer(device, h, header_size) };
                next = (next + 1) % CHUNKS;
            }
            tick();
            std::thread::sleep(Duration::from_millis(30));
        }
        // Reset returns the buffers being filled, with what they hold, in order.
        unsafe {
            waveInStop(device);
            waveInReset(device);
        }
        for _ in 0..CHUNKS {
            if headers[next].dwFlags & WHDR_DONE != 0 {
                take(&headers[next], &buffers[next], &mut pcm);
                headers[next].dwFlags &= !WHDR_DONE;
            }
            next = (next + 1) % CHUNKS;
        }
        Ok(())
    })();
    unsafe {
        waveInReset(device);
        for h in headers.iter_mut() {
            waveInUnprepareHeader(device, &mut **h, header_size);
        }
        waveInClose(device);
    }
    level.store(0, Ordering::Relaxed);
    result?;
    if pcm.len() < SAMPLE_RATE / 2 {
        return Ok(Vec::new());
    }
    // Pure silence (all zeros or almost): Windows gives that when desktop apps may not use the microphone.
    if peak < 0.002 {
        bail!("{}", crate::i18n::tr("The microphone gave only silence. Check that it is not muted and that Windows Settings → Privacy → Microphone lets desktop apps use it."));
    }
    Ok(pcm)
}

#[cfg(not(windows))]
fn record(_stop: &AtomicBool, _level: &AtomicU32, _heard: &std::sync::Mutex<Vec<f32>>, _tick: impl Fn()) -> Result<Vec<f32>> {
    bail!("{}", crate::i18n::tr("No microphone was found. Connect one, or switch it on in the Windows sound settings."))
}

/// The text of 16 kHz mono speech. The model is read into memory first: whisper.cpp would open the file by an ANSI
/// path, which fails for a user folder with letters outside the system code page (e.g. a user named Łucja on English Windows).
pub fn recognise(model_file: &std::path::Path, language: &str, pcm: &[f32], cancel: &Arc<AtomicBool>) -> Result<String> {
    if pcm.is_empty() {
        return Ok(String::new());
    }
    let context = load(model_file)?;
    recognise_with(&context, language, pcm, cancel)
}

/// The speech model in memory (read as bytes: see `recognise`).
fn load(model_file: &std::path::Path) -> Result<whisper_rs::WhisperContext> {
    if !cpu_supported() {
        bail!("{}", crate::i18n::tr("This computer's processor lacks the AVX2 instructions that speech recognition needs."));
    }
    let bytes = std::fs::read(model_file).with_context(|| model_file.display().to_string())?;
    whisper_rs::WhisperContext::new_from_buffer_with_params(&bytes, whisper_rs::WhisperContextParameters::default())
        .map_err(|e| anyhow::anyhow!("{}", crate::i18n::fmt("The speech model could not be loaded ({0}). Delete {1} and download it again.", &[e.to_string(), model_file.display().to_string()])))
}

/// Seconds of speech recognised at a time while speaking; longer speech is recognised in parts of this length.
const LIVE_PART: usize = 20;

/// While recording: about every 0.7 s what was said so far is recognised and handed to `show` (LC-485). Parts of
/// LIVE_PART seconds are recognised once and kept, so each pass stays short. Ends when the recording stops and
/// returns the loaded model for the final recognition (None when it could not be loaded).
fn live_text(model_file: &std::path::Path, language: &str, heard: &std::sync::Mutex<Vec<f32>>, stop: &Arc<AtomicBool>, show: impl Fn(String)) -> Option<whisper_rs::WhisperContext> {
    let context = load(model_file).ok()?;
    let (mut kept, mut kept_until, mut shown_at) = (String::new(), 0usize, 0usize);
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(700));
        let so_far = heard.lock().map(|h| h.clone()).unwrap_or_default();
        // Something new worth a pass: at least half a second in all and 0.4 s since the last one.
        if so_far.len() < SAMPLE_RATE / 2 || so_far.len() < shown_at + SAMPLE_RATE * 4 / 10 {
            continue;
        }
        shown_at = so_far.len();
        while so_far.len() - kept_until > LIVE_PART * SAMPLE_RATE && !stop.load(Ordering::Relaxed) {
            let part = &so_far[kept_until..kept_until + LIVE_PART * SAMPLE_RATE];
            if let Ok(text) = recognise_with(&context, language, part, stop) {
                kept = [kept.as_str(), text.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" ");
            }
            kept_until += LIVE_PART * SAMPLE_RATE;
        }
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(text) = recognise_with(&context, language, &so_far[kept_until..], stop) {
            let all = [kept.as_str(), text.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" ");
            if !all.is_empty() {
                show(all);
            }
        }
    }
    Some(context)
}

/// The text of 16 kHz mono speech with a loaded model.
fn recognise_with(context: &whisper_rs::WhisperContext, language: &str, pcm: &[f32], cancel: &Arc<AtomicBool>) -> Result<String> {
    if pcm.is_empty() {
        return Ok(String::new());
    }
    let mut state = context.create_state().map_err(|e| anyhow::anyhow!("whisper: {e}"))?;
    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some(language));
    params.set_n_threads(std::thread::available_parallelism().map_or(4, |n| n.get().min(8)) as i32);
    params.set_no_timestamps(true);
    params.set_suppress_nst(true);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    // A short recording needs only part of the 30-second window: several times faster, the same text.
    let seconds = pcm.len() / SAMPLE_RATE;
    if seconds < 28 {
        params.set_audio_ctx((seconds as i32 + 2) * 50 + 200);
    }
    // Cancel breaks the recognition off. Not `set_abort_callback_safe` of whisper-rs 0.16: its trampoline reads the
    // closure as the wrong type and whisper stopped at once ("failed to encode") on most runs.
    unsafe extern "C" fn cancelled(flag: *mut std::ffi::c_void) -> bool {
        unsafe { (*(flag as *const AtomicBool)).load(Ordering::Relaxed) }
    }
    // SAFETY: the flag lives in `cancel`, borrowed until `full` returns.
    unsafe {
        params.set_abort_callback(Some(cancelled));
        params.set_abort_callback_user_data(Arc::as_ptr(cancel) as *mut std::ffi::c_void);
    }
    state.full(params, pcm).map_err(|e| anyhow::anyhow!("whisper: {e}"))?;
    let mut text = String::new();
    for i in 0..state.full_n_segments() {
        if let Some(segment) = state.get_segment(i) {
            text.push_str(&segment.to_str_lossy().unwrap_or_default());
        }
    }
    Ok(clean(&text))
}

/// The text without the markers whisper writes for sounds that are not speech ("[BLANK_AUDIO]", "(music)", "[Musik]").
fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = out.trim();
    if trimmed.starts_with('(') && trimmed.ends_with(')') && !trimmed[1..].contains('(') {
        return String::new();
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_of_non_speech_are_removed() {
        assert_eq!(clean(" [BLANK_AUDIO]"), "");
        assert_eq!(clean(" (muzyka)"), "");
        assert_eq!(clean(" Dodaj diagram [Musik] do pakietu."), "Dodaj diagram do pakietu.");
        assert_eq!(clean(" Add a class (Order) to the package."), "Add a class (Order) to the package.");
    }

    #[test]
    fn language_follows_the_interface_unless_chosen() {
        crate::i18n::set(crate::i18n::Lang::De);
        assert_eq!(language(""), "de");
        assert_eq!(language("fr"), "fr");
        assert_eq!(language("pl"), "pl");
        crate::i18n::set(crate::i18n::Lang::En);
    }

    #[test]
    fn models_have_their_checksums() {
        assert_eq!(model("base").file, "ggml-base-q5_1.bin");
        assert_eq!(model("unknown").name, "small");
        assert!(MODELS.iter().all(|m| m.sha256.len() == 64));
    }

    /// The smaller model from Hugging Face, checked by its SHA-256 (about 60 MB).
    #[test]
    #[ignore = "downloads 60 MB"]
    fn model_is_downloaded_and_checked() {
        let dir = std::env::temp_dir().join(format!("loonar-speech-{}", std::process::id()));
        let last = std::sync::Mutex::new((0, 0));
        let path = download_to(model("base"), dir.clone(), &AtomicBool::new(false), |done, total| *last.lock().unwrap() = (done, total)).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 59_707_625);
        assert!(last.lock().unwrap().1 == 59_707_625);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The default microphone, recorded for a second: on a computer without one the message says so.
    #[test]
    #[ignore = "uses the microphone of the computer"]
    fn microphone_records_or_says_why_not() {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            flag.store(true, Ordering::Relaxed);
        });
        match record(&stop, &AtomicU32::new(0), &std::sync::Mutex::new(Vec::new()), || {}) {
            Ok(pcm) => println!("recorded {} samples", pcm.len()),
            Err(e) => println!("no recording: {e:#}"),
        }
    }

    /// A 16 kHz mono WAV recognised with a model on disk: LC_SPEECH_MODEL=<model.bin> LC_SPEECH_WAV=<file.wav>
    /// LC_SPEECH_LANG=pl LC_SPEECH_EXPECT="text" cargo test speech_file -- --ignored
    #[test]
    #[ignore = "needs a speech model and a recording"]
    fn speech_file_is_recognised() {
        let model = std::env::var("LC_SPEECH_MODEL").unwrap();
        let wav = std::fs::read(std::env::var("LC_SPEECH_WAV").unwrap()).unwrap();
        let pcm: Vec<f32> = wav[44..].chunks_exact(2).map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0).collect();
        let lang = std::env::var("LC_SPEECH_LANG").unwrap_or_else(|_| "en".into());
        let started = Instant::now();
        let text = recognise(std::path::Path::new(&model), &lang, &pcm, &Arc::new(AtomicBool::new(false))).unwrap();
        println!("{:?}: {text}", started.elapsed());
        if let Ok(expect) = std::env::var("LC_SPEECH_EXPECT") {
            let norm = |s: &str| s.to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>();
            assert_eq!(norm(&text), norm(&expect));
        }
    }

    /// LC-485: while "speaking" (the recording fed in real time), partial texts come and grow before the end:
    /// LC_SPEECH_MODEL=… LC_SPEECH_WAV=… LC_SPEECH_LANG=pl cargo test live_text_grows -- --ignored --nocapture
    #[test]
    #[ignore = "needs a speech model and a recording"]
    fn live_text_grows() {
        let model = std::path::PathBuf::from(std::env::var("LC_SPEECH_MODEL").unwrap());
        let wav = std::fs::read(std::env::var("LC_SPEECH_WAV").unwrap()).unwrap();
        let pcm: Vec<f32> = wav[44..].chunks_exact(2).map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0).collect();
        let lang: &'static str = Box::leak(std::env::var("LC_SPEECH_LANG").unwrap_or_else(|_| "en".into()).into_boxed_str());
        let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let shown = Arc::new(std::sync::Mutex::new(Vec::<(f32, String)>::new()));
        let started = Instant::now();
        let live = {
            let (heard, stop, shown) = (Arc::clone(&heard), Arc::clone(&stop), Arc::clone(&shown));
            std::thread::spawn(move || super::live_text(&model, lang, &heard, &stop, |t| shown.lock().unwrap().push((started.elapsed().as_secs_f32(), t))))
        };
        for chunk in pcm.chunks(SAMPLE_RATE / 10) {
            heard.lock().unwrap().extend_from_slice(chunk);
            std::thread::sleep(Duration::from_millis(100));
        }
        let spoken = started.elapsed().as_secs_f32();
        stop.store(true, Ordering::Relaxed);
        assert!(live.join().unwrap().is_some(), "the loaded model is handed over");
        let shown = shown.lock().unwrap().clone();
        for (at, text) in &shown {
            println!("{at:5.1}s  {text}");
        }
        println!("speech ended at {spoken:.1}s");
        assert!(shown.iter().any(|(at, _)| *at < spoken), "something was shown while speaking");
    }
}
