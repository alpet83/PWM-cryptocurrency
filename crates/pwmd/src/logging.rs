//! tracing wiring for pwmd: filters, rotating files, and structured TX traces.

use crate::config::{LogFileMode, LoggingConfig};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::{debug, error, info, Level, Metadata};
#[cfg(test)]
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::writer::{MakeWriter, MakeWriterExt};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;
use tracing_subscriber::Registry;

pub(crate) trait LogFilterCtl: Send + Sync {
    fn baseline_spec(&self) -> String;
    fn apply_spec(&self, spec: &str) -> Result<(), String>;

    fn apply_baseline(&self) -> Result<(), String> {
        self.apply_spec(&self.baseline_spec())
    }
}

pub(crate) type LogFilterCtlRef = Arc<dyn LogFilterCtl>;

#[derive(Clone)]
struct RuntimeLogCtl {
    baseline: String,
    reload: reload::Handle<EnvFilter, Registry>,
}

impl LogFilterCtl for RuntimeLogCtl {
    fn baseline_spec(&self) -> String {
        self.baseline.clone()
    }

    fn apply_spec(&self, spec: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(spec)
            .map_err(|e| format!("invalid runtime log filter {spec:?}: {e}"))?;
        self.reload
            .reload(filter)
            .map_err(|e| format!("runtime log filter reload failed: {e}"))
    }
}

static LOG_CTL: OnceLock<LogFilterCtlRef> = OnceLock::new();

pub(crate) fn runtime_log_ctl() -> Option<LogFilterCtlRef> {
    LOG_CTL.get().cloned()
}

pub(crate) fn ovr_filter_spec(base: &str, level: &str, focus: &str) -> String {
    if focus == "all" {
        return level.to_string();
    }
    let mut spec = base.trim().to_string();
    for target in focus_targets(focus) {
        if !spec.is_empty() {
            spec.push(',');
        }
        spec.push_str(target);
        spec.push('=');
        spec.push_str(level);
    }
    if spec.is_empty() {
        level.to_string()
    } else {
        spec
    }
}

fn focus_targets(focus: &str) -> &'static [&'static str] {
    match focus {
        "transport:peers" => &["pwmd::peer"],
        "sync:live" => &["pwmd::sync"],
        "seal:loop" => &["pwmd::lifecycle", "pwmd::lease"],
        "snapshot" => &["pwmd::snapshot", "pwmd::startup::snapshot"],
        "api" => &["pwmd::api"],
        _ => &[],
    }
}

fn startup_filter() -> (String, EnvFilter) {
    let from_env = std::env::var("RUST_LOG")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if let Some(spec) = from_env {
        match EnvFilter::try_new(spec.clone()) {
            Ok(filter) => return (spec, filter),
            Err(err) => {
                eprintln!("warning: invalid RUST_LOG={spec:?}; using debug fallback: {err}");
            }
        }
    }
    ("debug".to_string(), EnvFilter::new("debug"))
}

#[cfg(test)]
pub(crate) fn mk_test_log_ctl(base: &str) -> LogFilterCtlRef {
    Arc::new(TestLogCtl {
        base: base.to_string(),
        active: Mutex::new(base.to_string()),
    })
}

#[cfg(test)]
struct TestLogCtl {
    base: String,
    active: Mutex<String>,
}

#[cfg(test)]
impl LogFilterCtl for TestLogCtl {
    fn baseline_spec(&self) -> String {
        self.base.clone()
    }

    fn apply_spec(&self, spec: &str) -> Result<(), String> {
        let mut guard = self
            .active
            .lock()
            .map_err(|_| "test log ctl mutex poisoned".to_string())?;
        *guard = spec.to_string();
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
pub struct NodeLogger;

impl NodeLogger {
    pub fn info(self, event: &str) {
        info!("{event}");
    }

    pub fn error(self, event: &str) {
        error!("{event}");
    }

    pub fn debug_tx(
        self,
        height: u64,
        tx_kind: &str,
        tx_id: &str,
        addr: &str,
        before: u128,
        after: u128,
    ) {
        let delta = if after >= before {
            format!("+{}", after - before)
        } else {
            format!("-{}", before - after)
        };
        debug!(
            height,
            tx_kind,
            tx_id,
            addr,
            bal_before = before,
            bal_after = after,
            bal_delta = delta,
            "tx_included"
        );
    }
}

pub fn logger() -> NodeLogger {
    NodeLogger
}

#[derive(Clone, Copy)]
struct PwmdEventFormat {
    ansi: bool,
}

impl<S, N> FormatEvent<S, N> for PwmdEventFormat
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        let mut v = EventFieldVisitor::default();
        event.record(&mut v);
        let msg = v.message.take().unwrap_or_else(|| "event".to_string());
        let line = format_event_line(event.metadata().level(), &msg, &v.fields, self.ansi);
        if is_progress_console_line(&line) {
            write!(writer, "{line}")
        } else {
            writeln!(writer, "{line}")
        }
    }
}

#[derive(Default)]
struct EventFieldVisitor {
    message: Option<String>,
    fields: Vec<(String, String)>,
}

impl Visit for EventFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let key = field.name();
        let val = format!("{value:?}");
        self.push_field(key, val);
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.push_field(field.name(), value.to_string());
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.push_field(field.name(), value.to_string());
    }
}

impl EventFieldVisitor {
    fn push_field(&mut self, key: &str, value: String) {
        if key == "message" {
            self.message = Some(value);
            return;
        }
        self.fields.push((key.to_string(), value));
    }
}

fn format_event_line(level: &Level, msg: &str, fields: &[(String, String)], ansi: bool) -> String {
    let ts = now_hms_millis();
    let tag = level_tag(level);
    let tag = if ansi {
        color_level_tag(level, tag).to_string()
    } else {
        tag.to_string()
    };
    let msg = if ansi {
        color_message(level, msg)
    } else {
        msg.to_string()
    };
    let mut line = format!("[{ts}] {tag}: {msg}");
    if !fields.is_empty() {
        line.push_str(" | ");
        for (idx, (k, v)) in fields.iter().enumerate() {
            if idx > 0 {
                line.push(' ');
            }
            line.push_str(k);
            line.push('=');
            if ansi {
                line.push_str(&color_field_value(v));
            } else {
                line.push_str(v);
            }
        }
    }
    line
}

fn level_tag(level: &Level) -> &'static str {
    match *level {
        Level::TRACE => "#TRACE",
        Level::DEBUG => "#DEBUG",
        Level::INFO => "#INFO",
        Level::WARN => "#WARN",
        Level::ERROR => "#ERROR",
    }
}

fn color_level_tag(level: &Level, tag: &'static str) -> &'static str {
    match *level {
        Level::ERROR => "\x1b[91m#ERROR\x1b[0m",
        Level::WARN => "\x1b[31m#WARN\x1b[0m",
        _ => tag,
    }
}

fn color_message(level: &Level, msg: &str) -> String {
    let color = match *level {
        Level::TRACE | Level::DEBUG => "\x1b[93m", // stage/matter events
        Level::INFO => "\x1b[94m",                 // regular informational messages
        _ => "",
    };
    tint_message_keep_numbers(msg, color)
}

fn color_field_value(value: &str) -> String {
    if looks_like_structure(value) {
        return wrap_color(value, "\x1b[97m");
    }
    if looks_like_id(value) {
        return wrap_color(value, "\x1b[92m");
    }
    if looks_like_numeric_value(value) {
        return highlight_numbers(value);
    }
    wrap_color(value, "\x1b[92m")
}

fn wrap_color(input: &str, color: &str) -> String {
    format!("{color}{input}\x1b[0m")
}

fn tint_message_keep_numbers(input: &str, color: &str) -> String {
    if color.is_empty() {
        return highlight_numbers(input);
    }
    let highlighted = highlight_numbers(input);
    let mut tinted = String::with_capacity(highlighted.len() + color.len() * 2 + 4);
    tinted.push_str(color);
    tinted.push_str(&highlighted.replace("\x1b[0m", &format!("\x1b[0m{color}")));
    tinted.push_str("\x1b[0m");
    tinted
}

fn now_hms_millis() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let sec_day = d.as_secs() % 86_400;
    let h = sec_day / 3_600;
    let m = (sec_day % 3_600) / 60;
    let s = sec_day % 60;
    let ms = d.subsec_millis();
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn highlight_numbers(input: &str) -> String {
    if looks_like_id(input) {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len() + 8);
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        if is_number_start(&chars, i) {
            let start = i;
            i += 1;
            while i < chars.len() && is_number_continue(chars[i]) {
                i += 1;
            }
            let num: String = chars[start..i].iter().collect();
            out.push_str("\x1b[95m");
            out.push_str(&num);
            out.push_str("\x1b[0m");
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn is_number_start(chars: &[char], i: usize) -> bool {
    let ch = chars[i];
    if ch.is_ascii_digit() {
        return true;
    }
    if matches!(ch, '+' | '-') {
        return i + 1 < chars.len() && chars[i + 1].is_ascii_digit();
    }
    false
}

fn is_number_continue(ch: char) -> bool {
    ch.is_ascii_digit() || ch == '.'
}

/// Returns true if the string looks like an account id or hex hash.
fn looks_like_id(input: &str) -> bool {
    if let Some(hex) = input.strip_prefix("0x") {
        return hex.len() >= 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
    }
    if input.len() >= 12 && input.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if input.len() >= 16
        && input
            .chars()
            .all(|c| "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(c))
    {
        return true;
    }
    input.len() >= 16
        && input
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-'))
}

fn looks_like_structure(input: &str) -> bool {
    let t = input.trim();
    (t.starts_with('{') && t.ends_with('}')) || (t.starts_with('[') && t.ends_with(']'))
}

fn looks_like_numeric_value(input: &str) -> bool {
    if looks_like_id(input) {
        return false;
    }
    input.chars().any(|c| c.is_ascii_digit())
}

fn console_ansi_enabled(cfg: &LoggingConfig, console_is_tty: bool) -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    cfg.console_color.use_ansi(console_is_tty)
}

pub fn init_logging(
    cfg: &LoggingConfig,
    console_is_tty: bool,
    runtime_node_id: Option<&str>,
) -> Result<(), String> {
    cfg.validate()?;
    let (baseline, filter) = startup_filter();
    let (filter, reload_handle) = reload::Layer::new(filter);
    let console_ansi = console_ansi_enabled(cfg, console_is_tty);
    let mk_console = || {
        let out = std::io::stdout
            .with_max_level(Level::INFO)
            .with_filter(|meta: &Metadata<'_>| !is_peer_target(meta));
        let err = std::io::stderr
            .with_min_level(Level::WARN)
            .with_filter(|meta: &Metadata<'_>| !is_peer_target(meta));
        tracing_subscriber::fmt::layer()
            .event_format(PwmdEventFormat { ansi: console_ansi })
            .with_ansi(console_ansi)
            .with_writer(out.or_else(err))
    };
    let main_file_layer = build_file_layer(
        &cfg.log_dir,
        &cfg.file_template,
        &cfg.log_name,
        runtime_node_id,
        cfg.rotate_size_mb,
        cfg.rotate_max_files,
        cfg.file_mode,
        "main",
    )?
    .map(|writer| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .event_format(PwmdEventFormat { ansi: false })
            .with_writer(writer.with_filter(|meta: &Metadata<'_>| !is_peer_target(meta)))
    });
    let peer_file_layer = build_file_layer(
        &cfg.log_dir,
        &cfg.peer_file_template,
        "pwmd-peer",
        runtime_node_id,
        cfg.rotate_size_mb,
        cfg.rotate_max_files,
        cfg.peer_file_mode,
        "peer",
    )?
    .map(|writer| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .event_format(PwmdEventFormat { ansi: false })
            .with_writer(writer.with_filter(is_peer_target))
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(mk_console())
        .with(main_file_layer)
        .with(peer_file_layer)
        .init();
    let _ = LOG_CTL.set(Arc::new(RuntimeLogCtl {
        baseline,
        reload: reload_handle,
    }));
    Ok(())
}

fn is_peer_target(meta: &Metadata<'_>) -> bool {
    meta.target().starts_with("pwmd::peer")
}

fn build_file_layer(
    log_dir: &Path,
    template: &str,
    log_name: &str,
    runtime_node_id: Option<&str>,
    rotate_size_mb: u64,
    rotate_max_files: usize,
    mode: LogFileMode,
    sink_name: &str,
) -> Result<Option<RotatingWriter>, String> {
    let path = expand_log_template_path(log_dir, template, log_name, runtime_node_id)?;
    let max_size = rotate_size_mb * 1024 * 1024;
    match mode {
        LogFileMode::Off => Ok(None),
        LogFileMode::Required => {
            let rotating =
                RotatingFile::new(path, max_size, rotate_max_files).map_err(|e| e.to_string())?;
            Ok(Some(RotatingWriter::new(
                rotating,
                FileFailPolicy::FailHard,
            )))
        }
        LogFileMode::On => match RotatingFile::new(path, max_size, rotate_max_files) {
            Ok(rotating) => Ok(Some(RotatingWriter::new(rotating, FileFailPolicy::Degrade))),
            Err(err) => {
                eprintln!(
                    "warning: {sink_name} file logger disabled (mode=on, setup failed): {err}. continuing"
                );
                Ok(None)
            }
        },
    }
}

pub(crate) fn expand_log_template_path(
    log_dir: &Path,
    template: &str,
    log_name: &str,
    runtime_node_id: Option<&str>,
) -> Result<PathBuf, String> {
    if log_name.trim().is_empty() {
        return Err("log name must not be empty".to_string());
    }
    let (date, time, datetime, ut) = now_tokens()?;

    let node_id = sanitize_template_token(runtime_node_id.unwrap_or("node-unknown"));
    let expanded = template
        .replace("{date}", &date)
        .replace("{time}", &time)
        .replace("{datetime}", &datetime)
        .replace("~UT", &ut)
        .replace("{log_name}", log_name)
        .replace("{node_id}", &node_id)
        .replace("{pid}", &std::process::id().to_string());

    let rel = PathBuf::from(expanded);
    if rel.as_os_str().is_empty() {
        return Err("log template expanded to empty path".to_string());
    }
    for c in rel.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => {
                return Err("log template must be a relative path".to_string());
            }
            Component::ParentDir => return Err("log template must not contain '..'".to_string()),
            _ => {}
        }
    }
    Ok(log_dir.join(rel))
}

fn sanitize_template_token(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "node-unknown".to_string()
    } else {
        out
    }
}

fn now_tokens() -> Result<(String, String, String, String), String> {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("clock error: {e}"))?;
    let now = dur.as_secs();
    let days = (now / 86_400) as i64;
    let sec_day = now % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = sec_day / 3_600;
    let minute = (sec_day % 3_600) / 60;
    let second = sec_day % 60;
    let date = format!("{year:04}-{month:02}-{day:02}");
    let time = format!("{hour:02}{minute:02}{second:02}");
    let datetime = format!("{date}T{time}");
    let ut = format!(
        "{hour:02}:{minute:02}:{second:02}.{:03}",
        dur.subsec_millis()
    );
    Ok((date, time, datetime, ut))
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

#[derive(Clone)]
struct RotatingWriter {
    inner: Arc<Mutex<RotatingFile>>,
    fail_policy: FileFailPolicy,
}

impl RotatingWriter {
    fn new(file: RotatingFile, fail_policy: FileFailPolicy) -> Self {
        Self {
            inner: Arc::new(Mutex::new(file)),
            fail_policy,
        }
    }
}

impl<'a> MakeWriter<'a> for RotatingWriter {
    type Writer = RotatingGuard;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingGuard {
            inner: Arc::clone(&self.inner),
            fail_policy: self.fail_policy,
        }
    }
}

struct RotatingGuard {
    inner: Arc<Mutex<RotatingFile>>,
    fail_policy: FileFailPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileFailPolicy {
    Degrade,
    FailHard,
}

impl Write for RotatingGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if is_progress_line(buf) {
            return Ok(buf.len());
        }
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log file writer poisoned"))?;
        match guard.write(buf) {
            Ok(written) => Ok(written),
            Err(err) => match self.fail_policy {
                FileFailPolicy::FailHard => {
                    panic!("required log file sink failed during write/rotate: {err}");
                }
                FileFailPolicy::Degrade => {
                    guard.enter_degraded("write/rotate", &err);
                    Ok(buf.len())
                }
            },
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("log file writer poisoned"))?;
        match guard.flush() {
            Ok(()) => Ok(()),
            Err(err) => match self.fail_policy {
                FileFailPolicy::FailHard => {
                    panic!("required log file sink failed during flush: {err}");
                }
                FileFailPolicy::Degrade => {
                    guard.enter_degraded("flush", &err);
                    Ok(())
                }
            },
        }
    }
}

fn is_progress_line(buf: &[u8]) -> bool {
    let mut end = buf.len();
    while end > 0 && matches!(buf[end - 1], b'\n' | b' ' | b'\t') {
        end -= 1;
    }
    end > 0 && buf[end - 1] == b'\r'
}

fn is_progress_console_line(line: &str) -> bool {
    line.ends_with('\r')
}

struct RotatingFile {
    path: PathBuf,
    file: File,
    current_size: u64,
    max_size: u64,
    max_files: usize,
    degraded: bool,
    #[cfg(test)]
    test_fail_remove_once: bool,
    #[cfg(test)]
    test_fail_rename_once: bool,
}

impl RotatingFile {
    fn new(path: PathBuf, max_size: u64, max_files: usize) -> io::Result<Self> {
        if max_size == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "max_size must be > 0",
            ));
        }
        if max_files == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "max_files must be > 0",
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let current_size = file.metadata()?.len();
        Ok(Self {
            path,
            file,
            current_size,
            max_size,
            max_files,
            degraded: false,
            #[cfg(test)]
            test_fail_remove_once: false,
            #[cfg(test)]
            test_fail_rename_once: false,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        let oldest = self.rotated_path(self.max_files);
        if oldest.exists() {
            self.remove_file(&oldest)?;
        }
        for n in (1..self.max_files).rev() {
            let from = self.rotated_path(n);
            let to = self.rotated_path(n + 1);
            if from.exists() {
                self.rename(&from, &to)?;
            }
        }
        if self.path.exists() {
            self.rename(&self.path.clone(), &self.rotated_path(1))?;
        }
        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        self.current_size = 0;
        Ok(())
    }

    fn rotated_path(&self, idx: usize) -> PathBuf {
        PathBuf::from(format!("{}.{}", self.path.to_string_lossy(), idx))
    }

    fn enter_degraded(&mut self, op: &str, err: &io::Error) {
        if !self.degraded {
            eprintln!(
                "warning: file logger degraded after {op} error: {err}. continuing in console-only mode"
            );
            self.degraded = true;
        }
    }

    fn remove_file(&mut self, path: &Path) -> io::Result<()> {
        #[cfg(test)]
        {
            if self.test_fail_remove_once {
                self.test_fail_remove_once = false;
                return Err(io::Error::other("injected remove_file failure"));
            }
        }
        fs::remove_file(path)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        #[cfg(test)]
        {
            if self.test_fail_rename_once {
                self.test_fail_rename_once = false;
                return Err(io::Error::other("injected rename failure"));
            }
        }
        fs::rename(from, to)
    }

    #[cfg(test)]
    fn inject_rename_failure(&mut self) {
        self.test_fail_rename_once = true;
    }

    #[cfg(test)]
    fn is_degraded(&self) -> bool {
        self.degraded
    }
}

impl Write for RotatingFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.degraded {
            return Ok(buf.len());
        }
        let incoming = buf.len() as u64;
        if self.current_size > 0 && self.current_size + incoming > self.max_size {
            self.rotate()?;
        }
        let written = self.file.write(buf)?;
        self.current_size = self.current_size.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.degraded {
            return Ok(());
        }
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LoggingConfig;
    use std::io;
    use std::panic::AssertUnwindSafe;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let p = std::env::temp_dir().join(format!("pwmd-logger-{tag}-{ts}"));
        fs::create_dir_all(&p).expect("mkdir");
        p
    }

    #[derive(Clone, Default)]
    struct BufSink {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    struct BufWriter {
        inner: Arc<Mutex<Vec<u8>>>,
    }

    impl<'a> MakeWriter<'a> for BufSink {
        type Writer = BufWriter;

        fn make_writer(&'a self) -> Self::Writer {
            BufWriter {
                inner: Arc::clone(&self.inner),
            }
        }
    }

    impl io::Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut g = self
                .inner
                .lock()
                .map_err(|_| io::Error::other("buffer lock poisoned"))?;
            g.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn template_expands_subdir_placeholders() {
        let root = temp_dir("template");
        let p = expand_log_template_path(
            &root,
            "{date}/{log_name}-{node_id}-{time}.log",
            "pwmd",
            Some("node-a"),
        )
        .expect("expand");
        assert!(p.starts_with(&root));
        assert!(p.extension().and_then(|v| v.to_str()) == Some("log"));
    }

    #[test]
    fn template_expands_ut_placeholder() {
        let root = temp_dir("template-ut");
        let p = expand_log_template_path(&root, "{date}/{log_name}_~UT.log", "pwmd", None)
            .expect("expand");
        let name = p
            .file_name()
            .and_then(|v| v.to_str())
            .expect("utf8 file name");
        assert!(name.starts_with("pwmd_"));
        assert!(name.ends_with(".log"));
        assert!(!name.contains("~UT"));
        let time = name.trim_start_matches("pwmd_").trim_end_matches(".log");
        assert_eq!(time.len(), "00:00:00.000".len());
        assert!(time.chars().nth(2) == Some(':'));
        assert!(time.chars().nth(5) == Some(':'));
        assert!(time.chars().nth(8) == Some('.'));
    }

    #[test]
    fn template_rejects_parent_dir() {
        let root = temp_dir("safety-parent");
        let err =
            expand_log_template_path(&root, "../oops.log", "pwmd", None).expect_err("must fail");
        assert!(err.contains("must not contain '..'"));
    }

    #[test]
    fn template_rejects_absolute() {
        let root = temp_dir("safety-abs");
        let err =
            expand_log_template_path(&root, "/tmp/oops.log", "pwmd", None).expect_err("must fail");
        assert!(err.contains("relative"));
    }

    /// `{node_id}` placeholder sanitizes odd characters (formerly `template_expands_node_id_placeholder_with_sanitization`).
    #[test]
    fn tmpl_node_id_sanitize_ok() {
        let root = temp_dir("template-node-id");
        let p = expand_log_template_path(
            &root,
            "{date}/{log_name}-{node_id}-{time}.log",
            "pwmd",
            Some("node:/with*bad chars"),
        )
        .expect("expand");
        let name = p
            .file_name()
            .and_then(|v| v.to_str())
            .expect("utf8 file name");
        assert!(name.starts_with("pwmd-node__with_bad_chars-"));
        assert!(name.ends_with(".log"));
    }

    /// Unknown node ids expand to deterministic fallback token (formerly `template_uses_node_id_fallback_when_unavailable`).
    #[test]
    fn tmpl_node_id_fallback_ok() {
        let root = temp_dir("template-node-id-fallback");
        let p = expand_log_template_path(
            &root,
            "{date}/{log_name}-{node_id}-{time}.log",
            "pwmd",
            None,
        )
        .expect("expand");
        let name = p
            .file_name()
            .and_then(|v| v.to_str())
            .expect("utf8 file name");
        assert!(name.starts_with("pwmd-node-unknown-"));
        assert!(name.ends_with(".log"));
    }

    /// `ConsoleColorMode::Auto` skips ANSI without TTY (formerly `console_color_auto_non_tty_is_plain`).
    #[test]
    fn color_auto_no_tty_plain() {
        assert!(!crate::config::ConsoleColorMode::Auto.use_ansi(false));
        assert!(crate::config::ConsoleColorMode::Always.use_ansi(false));
        assert!(!crate::config::ConsoleColorMode::Never.use_ansi(true));
    }

    #[test]
    fn formatter_plain_contract() {
        let line = format_event_line(
            &Level::INFO,
            "tx_included",
            &[
                ("height".to_string(), "42".to_string()),
                ("tx_id".to_string(), "0xabc123".to_string()),
            ],
            false,
        );
        assert!(line.contains("#INFO: tx_included | height=42 tx_id=0xabc123"));
        assert!(line.starts_with('['));
        assert!(line.contains("] #INFO:"));
    }

    /// Warn/error tags gain distinct palettes (formerly `formatter_colors_warn_and_error_tags`).
    #[test]
    fn fmt_clr_warn_err_tags() {
        let warn = format_event_line(&Level::WARN, "warn_event", &[], true);
        let err = format_event_line(&Level::ERROR, "err_event", &[], true);
        assert!(warn.contains("\x1b[31m#WARN\x1b[0m"));
        assert!(err.contains("\x1b[91m#ERROR\x1b[0m"));
    }

    /// Info/stage formatting highlights headline numerics (formerly `formatter_colors_info_and_stage_text`).
    #[test]
    fn fmt_clr_info_stage_nums() {
        let info = format_event_line(&Level::INFO, "sealed height=42", &[], true);
        let stage = format_event_line(&Level::DEBUG, "sync pass 7", &[], true);
        assert!(info.contains("\x1b[94m"));
        assert!(stage.contains("\x1b[93m"));
        assert!(info.contains("\x1b[95m42\x1b[0m"));
        assert!(stage.contains("\x1b[95m7\x1b[0m"));
    }

    /// Numeric brush hits message bodies and KV values (formerly `numeric_highlight_applies_to_message_and_values`).
    #[test]
    fn num_hi_msg_kv_values() {
        let line = format_event_line(
            &Level::INFO,
            "processed 17 items in 12.5 ms",
            &[("amount".to_string(), "9000".to_string())],
            true,
        );
        assert!(line.contains("\x1b[95m17\x1b[0m"));
        assert!(line.contains("\x1b[95m12.5\x1b[0m"));
        assert!(line.contains("amount=\x1b[95m9000\x1b[0m"));
    }

    /// Hex-like strings skip numeric repaint (formerly `numeric_highlight_skips_hash_like_tokens`).
    #[test]
    fn num_hi_skip_hash_like() {
        let line = format_event_line(
            &Level::INFO,
            "tx 0xabc123deadbeef hash",
            &[("id".to_string(), "7j5f3Q2x9kLmN8pR".to_string())],
            true,
        );
        assert!(!line.contains("\x1b[95m0xabc123deadbeef\x1b[0m"));
        assert!(line.contains("id=\x1b[92m7j5f3Q2x9kLmN8pR\x1b[0m"));
    }

    /// Field values pick green/gray/magenta palettes (formerly `formatter_field_value_palette_matches_style`).
    #[test]
    fn clr_fld_vals_ok() {
        let line = format_event_line(
            &Level::INFO,
            "status",
            &[
                ("s".to_string(), "ok".to_string()),
                ("n".to_string(), "42".to_string()),
                ("j".to_string(), "{\"a\":1}".to_string()),
            ],
            true,
        );
        assert!(line.contains("s=\x1b[92mok\x1b[0m"));
        assert!(line.contains("n=\x1b[95m42\x1b[0m"));
        assert!(line.contains("j=\x1b[97m{\"a\":1}\x1b[0m"));
    }

    /// Respect `NO_COLOR` even when output is TTY (formerly `no_color_disables_ansi_even_in_tty`).
    #[test]
    fn no_color_kills_tty_ansi() {
        let cfg = LoggingConfig::default();
        // SAFETY: test sets and restores process env for isolated assertion.
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }
        assert!(!console_ansi_enabled(&cfg, true));
        // SAFETY: test cleanup mirrors prior set_var for this key.
        unsafe {
            std::env::remove_var("NO_COLOR");
        }
    }

    #[test]
    fn logging_bounds_are_validated() {
        let mut cfg = LoggingConfig::default();
        cfg.rotate_size_mb = 0;
        assert!(cfg.validate().is_err());
        cfg.rotate_size_mb = 1;
        cfg.rotate_max_files = 0;
        assert!(cfg.validate().is_err());
    }

    /// Rotation honors max retained files after threshold (formerly `rotation_triggers_and_keeps_retention_cap`).
    #[test]
    fn rotate_keeps_retention_cap() {
        let root = temp_dir("rotate");
        let path = root.join("app.log");
        let mut w = RotatingFile::new(path.clone(), 24, 2).expect("writer");
        let chunk = b"0123456789\n";
        w.write_all(chunk).expect("write1");
        w.write_all(chunk).expect("write2");
        w.write_all(chunk).expect("write3");
        w.write_all(chunk).expect("write4");
        w.write_all(chunk).expect("write5");
        w.flush().expect("flush");
        assert!(path.exists());
        assert!(path.with_extension("log.1").exists());
        assert!(path.with_extension("log.2").exists());
        assert!(!path.with_extension("log.3").exists());
    }

    /// Rotate failure keeps prior bytes on primary log path (formerly `rotate_error_does_not_truncate_active_log`).
    #[test]
    fn rotate_err_keeps_tail() {
        let root = temp_dir("rotate-io-error");
        let path = root.join("app.log");
        let mut w = RotatingFile::new(path.clone(), 12, 2).expect("writer");
        w.write_all(b"0123456789\n").expect("seed write");
        w.inject_rename_failure();
        let err = w.rotate().expect_err("rotate must fail");
        assert!(err.to_string().contains("injected rename failure"));
        drop(w);
        let active = fs::read_to_string(&path).expect("read active");
        assert!(active.contains("0123456789"));
    }

    /// `Degrade` policy continues writes after rename errors (formerly `on_mode_degrades_after_rotate_error`).
    #[test]
    fn degrade_on_rotate_err() {
        let root = temp_dir("on-mode-degrade");
        let path = root.join("app.log");
        let rotating = RotatingFile::new(path, 12, 2).expect("writer");
        let writer = RotatingWriter::new(rotating, FileFailPolicy::Degrade);
        {
            let mut locked = writer.inner.lock().expect("lock");
            locked.write_all(b"0123456789\n").expect("seed write");
            locked.inject_rename_failure();
        }
        let mut guard = writer.make_writer();
        let written = guard.write(b"abcdefghij\n").expect("degraded write");
        assert_eq!(written, b"abcdefghij\n".len());
        let locked = writer.inner.lock().expect("lock");
        assert!(locked.is_degraded());
    }

    /// Required log mode panics on rotate failure mid-write (formerly `required_mode_panics_after_rotate_error`).
    #[test]
    fn hard_rotate_err_panic() {
        let root = temp_dir("required-mode-panic");
        let path = root.join("app.log");
        let rotating = RotatingFile::new(path, 12, 2).expect("writer");
        let writer = RotatingWriter::new(rotating, FileFailPolicy::FailHard);
        {
            let mut locked = writer.inner.lock().expect("lock");
            locked.write_all(b"0123456789\n").expect("seed write");
            locked.inject_rename_failure();
        }
        let mut guard = writer.make_writer();
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            let _ = guard.write(b"abcdefghij\n");
        }));
        assert!(result.is_err());
    }

    /// File sink drops carriage-return progress overlays (formerly `file_sink_skips_progress_lines_for_cr_variants`).
    #[test]
    fn omit_cr_prog_sink() {
        let root = temp_dir("skip-progress");
        let path = root.join("app.log");
        let rotating = RotatingFile::new(path.clone(), 1024, 2).expect("writer");
        let writer = RotatingWriter::new(rotating, FileFailPolicy::Degrade);

        let mut guard = writer.make_writer();
        guard
            .write_all(b"[00:00:00.000] #INFO: sealed height=42\n")
            .expect("regular write");
        for line in [
            b"[00:00:00.000] #INFO: sealed height=%d    \r".as_slice(),
            b"[00:00:00.000] #INFO: sealed height=%d    \r\n".as_slice(),
            b"[00:00:00.000] #INFO: sealed height=%d    \r \n".as_slice(),
        ] {
            guard.write_all(line).expect("progress write");
        }
        guard.flush().expect("flush");
        drop(guard);

        let file_data = fs::read_to_string(path).expect("read file");
        assert!(file_data.contains("#INFO: sealed height=42"));
        assert!(!file_data.contains("#INFO: sealed height=%d"));
    }

    /// Detect console progress lines terminated with CR (formerly `progress_console_line_has_no_extra_newline`).
    #[test]
    fn prog_cr_tail_hit() {
        assert!(is_progress_console_line("#INFO: sealed height=42    \r"));
        assert!(!is_progress_console_line("#INFO: sealed height=42"));
    }

    #[test]
    fn peer_sink_isolated_from_main() {
        let main = BufSink::default();
        let peer = BufSink::default();
        let sub = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_ansi(false)
                    .with_writer(main.clone())
                    .with_filter(filter_fn(|meta: &Metadata<'_>| !is_peer_target(meta))),
            )
            .with(
                tracing_subscriber::fmt::layer()
                    .without_time()
                    .with_ansi(false)
                    .with_writer(peer.clone())
                    .with_filter(filter_fn(is_peer_target)),
            );
        let _guard = tracing::subscriber::set_default(sub);
        tracing::info!(target: "pwmd::peer", "peer event");
        tracing::info!(target: "pwmd::node", "main event");

        let main_text = String::from_utf8(main.inner.lock().expect("main lock").clone()).unwrap();
        let peer_text = String::from_utf8(peer.inner.lock().expect("peer lock").clone()).unwrap();
        assert!(main_text.contains("main event"));
        assert!(!main_text.contains("peer event"));
        assert!(peer_text.contains("peer event"));
        assert!(!peer_text.contains("main event"));
    }
}
