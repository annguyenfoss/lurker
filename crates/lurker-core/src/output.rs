use crate::model::ColorMode;
use serde::{Deserialize, Serialize};
use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};

const ESC_RESET: &str = "\u{1b}[0m";
const ESC_BOLD_BLUE: &str = "\u{1b}[1;34m";
const ESC_BOLD_GREEN: &str = "\u{1b}[1;32m";
const ESC_BOLD_YELLOW: &str = "\u{1b}[1;33m";
const ESC_BOLD_RED: &str = "\u{1b}[1;31m";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputLevel {
    RawStdout,
    RawStderr,
    Message,
    Detail,
    Success,
    Warning,
    Error,
    Progress,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputEntry {
    pub level: OutputLevel,
    pub message: String,
}

impl OutputEntry {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: OutputLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Clone)]
pub struct Output {
    inner: Arc<dyn OutputSink>,
}

#[derive(Clone)]
pub struct OutputBuffer {
    inner: Arc<Mutex<Vec<OutputEntry>>>,
}

impl OutputBuffer {
    pub fn entries(&self) -> Vec<OutputEntry> {
        self.inner.lock().unwrap().clone()
    }
}

trait OutputSink: Send + Sync {
    fn stderr_is_tty(&self) -> bool;
    fn record(&self, level: OutputLevel, message: &str);
    fn raw_stdout(&self, value: &str);
    fn raw_stderr(&self, value: &str);
    fn progress(&self, message: &str);
    fn finish_progress(&self);
}

struct TerminalOutput {
    use_color_stdout: bool,
    use_color_stderr: bool,
    stderr_tty: bool,
}

struct BufferedOutput {
    lines: Arc<Mutex<Vec<OutputEntry>>>,
}

struct SilentOutput;

impl Output {
    pub fn new(color_mode: ColorMode) -> Self {
        let stdout_tty = io::stdout().is_terminal();
        let stderr_tty = io::stderr().is_terminal();

        let (use_color_stdout, use_color_stderr) = match color_mode {
            ColorMode::Always => (true, true),
            ColorMode::Auto => (stdout_tty, stderr_tty),
            ColorMode::Never => (false, false),
        };

        Self {
            inner: Arc::new(TerminalOutput {
                use_color_stdout,
                use_color_stderr,
                stderr_tty,
            }),
        }
    }

    pub fn buffered() -> (Self, OutputBuffer) {
        let lines = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                inner: Arc::new(BufferedOutput {
                    lines: Arc::clone(&lines),
                }),
            },
            OutputBuffer { inner: lines },
        )
    }

    pub fn silent() -> Self {
        Self {
            inner: Arc::new(SilentOutput),
        }
    }

    pub fn stderr_is_tty(&self) -> bool {
        self.inner.stderr_is_tty()
    }

    pub fn raw_stdout(&self, value: &str) {
        self.inner.raw_stdout(value);
    }

    pub fn raw_stderr(&self, value: &str) {
        self.inner.raw_stderr(value);
    }

    pub fn msg(&self, message: &str) {
        self.inner.record(OutputLevel::Message, message);
    }

    pub fn msg2(&self, message: &str) {
        self.inner.record(OutputLevel::Detail, message);
    }

    pub fn success(&self, message: &str) {
        self.inner.record(OutputLevel::Success, message);
    }

    pub fn warn(&self, message: &str) {
        self.inner.record(OutputLevel::Warning, message);
    }

    pub fn error(&self, message: &str) {
        self.inner.record(OutputLevel::Error, message);
    }

    pub fn progress(&self, message: &str) {
        self.inner.progress(message);
    }

    pub fn finish_progress(&self) {
        self.inner.finish_progress();
    }
}

impl OutputSink for TerminalOutput {
    fn stderr_is_tty(&self) -> bool {
        self.stderr_tty
    }

    fn record(&self, level: OutputLevel, message: &str) {
        match level {
            OutputLevel::Message => self.print_stdout(ESC_BOLD_GREEN, "==>", message),
            OutputLevel::Detail => self.print_stdout(ESC_BOLD_BLUE, "  ->", message),
            OutputLevel::Success => self.print_stdout(ESC_BOLD_GREEN, "==>", message),
            OutputLevel::Warning => self.print_stderr(ESC_BOLD_YELLOW, "warning:", message),
            OutputLevel::Error => self.print_stderr(ESC_BOLD_RED, "error:", message),
            OutputLevel::RawStdout => self.raw_stdout(message),
            OutputLevel::RawStderr => self.raw_stderr(message),
            OutputLevel::Progress => self.progress(message),
        }
    }

    fn raw_stdout(&self, value: &str) {
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(value.as_bytes());
        let _ = stdout.flush();
    }

    fn raw_stderr(&self, value: &str) {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(value.as_bytes());
        let _ = stderr.flush();
    }

    fn progress(&self, message: &str) {
        let mut stderr = io::stderr().lock();
        if self.stderr_tty {
            let _ = write!(stderr, "\r{}", message);
        } else {
            let _ = writeln!(stderr, "{}", message);
        }
        let _ = stderr.flush();
    }

    fn finish_progress(&self) {
        if self.stderr_tty {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr);
            let _ = stderr.flush();
        }
    }
}

impl TerminalOutput {
    fn print_stdout(&self, color: &str, prefix: &str, message: &str) {
        let mut stdout = io::stdout().lock();
        if self.use_color_stdout {
            let _ = writeln!(stdout, "{}{}{} {}", color, prefix, ESC_RESET, message);
        } else {
            let _ = writeln!(stdout, "{} {}", prefix, message);
        }
        let _ = stdout.flush();
    }

    fn print_stderr(&self, color: &str, prefix: &str, message: &str) {
        let mut stderr = io::stderr().lock();
        if self.use_color_stderr {
            let _ = writeln!(stderr, "{}{}{} {}", color, prefix, ESC_RESET, message);
        } else {
            let _ = writeln!(stderr, "{} {}", prefix, message);
        }
        let _ = stderr.flush();
    }
}

impl OutputSink for BufferedOutput {
    fn stderr_is_tty(&self) -> bool {
        false
    }

    fn record(&self, level: OutputLevel, message: &str) {
        self.lines.lock().unwrap().push(OutputEntry {
            level,
            message: message.to_string(),
        });
    }

    fn raw_stdout(&self, value: &str) {
        self.record(OutputLevel::RawStdout, value);
    }

    fn raw_stderr(&self, value: &str) {
        self.record(OutputLevel::RawStderr, value);
    }

    fn progress(&self, message: &str) {
        self.record(OutputLevel::Progress, message);
    }

    fn finish_progress(&self) {}
}

impl OutputSink for SilentOutput {
    fn stderr_is_tty(&self) -> bool {
        false
    }

    fn record(&self, _level: OutputLevel, _message: &str) {}

    fn raw_stdout(&self, _value: &str) {}

    fn raw_stderr(&self, _value: &str) {}

    fn progress(&self, _message: &str) {}

    fn finish_progress(&self) {}
}
