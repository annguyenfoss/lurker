mod cli;
mod error;
mod linux;
mod model;
mod output;
mod sha256;
mod system;
mod workflow;

use cli::{ParseFailure, ParseOutcome};
use output::Output;

pub fn main_entry() -> i32 {
    match cli::parse_env_args(std::env::args_os()) {
        Ok(ParseOutcome::Run(command)) => {
            let output = Output::new(command.color_mode);
            let error_output = output.clone();
            match workflow::run(command, output) {
                Ok(()) => 0,
                Err(err) => {
                    error_output.error(&err.message);
                    err.exit_code
                }
            }
        }
        Ok(ParseOutcome::Help(help)) => {
            let output = Output::new(help.color_mode);
            let rendered = cli::render_help(&help.script_name, help.color_mode, help.to_stdout);
            if help.to_stdout {
                output.raw_stdout(&rendered);
            } else {
                output.raw_stderr(&rendered);
            }
            0
        }
        Err(ParseFailure {
            message,
            exit_code,
            show_help,
            help_to_stdout,
            script_name,
            color_mode,
        }) => {
            let output = Output::new(color_mode);
            if show_help {
                let rendered = cli::render_help(&script_name, color_mode, help_to_stdout);
                if help_to_stdout {
                    output.raw_stdout(&rendered);
                } else {
                    output.raw_stderr(&rendered);
                }
            }

            if let Some(message) = message {
                output.error(&message);
            }

            exit_code
        }
    }
}
