mod cli;
mod file_cli;
#[cfg(not(target_os = "macos"))]
mod session_close;

use cli::{ParseFailure, ParseOutcome};
use lurker_core::{run, Output};

fn main() {
    if let Some(code) = lurker_core::run_macos_filesystem_supervisor_if_requested() {
        std::process::exit(code);
    }
    if let Some(code) = lurker_core::run_folder_supervisor_if_requested() {
        std::process::exit(code);
    }
    std::process::exit(main_entry());
}

fn main_entry() -> i32 {
    let args: Vec<_> = std::env::args_os().collect();
    if let Some(code) = file_cli::run_if_selected(args.get(1..).unwrap_or_default()) {
        return code;
    }
    match cli::parse_env_args(args) {
        Ok(ParseOutcome::Run(command)) => {
            let output = Output::new(command.color_mode);
            let error_output = output.clone();
            match run(command.command, output) {
                Ok(()) => 0,
                Err(err) => {
                    error_output.error(&err.message);
                    err.exit_code
                }
            }
        }
        Ok(ParseOutcome::ListNative(color_mode)) => {
            let output = Output::new(color_mode);
            match lurker_core::list_native_active_volumes() {
                Ok(volumes) if volumes.is_empty() => {
                    output.raw_stdout("No active native volumes.\n");
                    0
                }
                Ok(volumes) => {
                    for volume in volumes {
                        let source = volume.source_path.as_deref().unwrap_or(&volume.mapper_path);
                        let location = volume.mountpoint.as_ref().map_or_else(
                            || "unmounted, close pending".to_owned(),
                            |path| format!("mounted at {path:?}"),
                        );
                        output.raw_stdout(&format!("{source:?} {location}\n"));
                    }
                    0
                }
                Err(error) => {
                    output.error(&error.message);
                    error.exit_code
                }
            }
        }
        Ok(ParseOutcome::SessionClose {
            command,
            timeout,
            color_mode,
        }) => {
            #[cfg(not(target_os = "macos"))]
            {
                let output = Output::new(color_mode);
                match session_close::close(command, timeout, &output) {
                    Ok(()) => 0,
                    Err(err) => {
                        output.error(&err.message);
                        err.exit_code
                    }
                }
            }
            #[cfg(target_os = "macos")]
            {
                let _ = (command, timeout, color_mode);
                1
            }
        }
        Ok(ParseOutcome::Help(help)) => {
            let output = Output::new(help.color_mode);
            let mut rendered = cli::render_help(&help.script_name, help.color_mode, help.to_stdout);
            rendered.push_str(file_cli::HELP);
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
