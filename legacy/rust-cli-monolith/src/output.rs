use crate::model::ColorMode;
use std::io::{self, IsTerminal, Write};

const ESC_RESET: &str = "\u{1b}[0m";
const ESC_BOLD_BLUE: &str = "\u{1b}[1;34m";
const ESC_BOLD_GREEN: &str = "\u{1b}[1;32m";
const ESC_BOLD_YELLOW: &str = "\u{1b}[1;33m";
const ESC_BOLD_RED: &str = "\u{1b}[1;31m";

#[derive(Clone, Debug)]
pub struct Output {
    use_color_stdout: bool,
    use_color_stderr: bool,
    stderr_tty: bool,
}

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
            use_color_stdout,
            use_color_stderr,
            stderr_tty,
        }
    }

    pub fn stderr_is_tty(&self) -> bool {
        self.stderr_tty
    }

    pub fn raw_stdout(&self, value: &str) {
        let mut stdout = io::stdout().lock();
        let _ = stdout.write_all(value.as_bytes());
        let _ = stdout.flush();
    }

    pub fn raw_stderr(&self, value: &str) {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(value.as_bytes());
        let _ = stderr.flush();
    }

    pub fn msg(&self, message: &str) {
        self.print_stdout(ESC_BOLD_GREEN, "==>", message);
    }

    pub fn msg2(&self, message: &str) {
        self.print_stdout(ESC_BOLD_BLUE, "  ->", message);
    }

    pub fn success(&self, message: &str) {
        self.print_stdout(ESC_BOLD_GREEN, "==>", message);
    }

    pub fn warn(&self, message: &str) {
        self.print_stderr(ESC_BOLD_YELLOW, "warning:", message);
    }

    pub fn error(&self, message: &str) {
        self.print_stderr(ESC_BOLD_RED, "error:", message);
    }

    pub fn progress(&self, message: &str) {
        let mut stderr = io::stderr().lock();
        if self.stderr_tty {
            let _ = write!(stderr, "\r{}", message);
        } else {
            let _ = writeln!(stderr, "{}", message);
        }
        let _ = stderr.flush();
    }

    pub fn finish_progress(&self) {
        if self.stderr_tty {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr);
            let _ = stderr.flush();
        }
    }

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
