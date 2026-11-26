use lurker_core::file_crypto::{self, FileBackend, FileOperation, FileRequest};
use lurker_core::{AppError, AppResult, ColorMode, Output};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

pub const HELP: &str = "\nFile encryption:\n\
  lurker encrypt --type age|gnupg|openssl [CREDENTIALS] INPUT OUTPUT\n\
  lurker decrypt --type age|gnupg|openssl [CREDENTIALS] INPUT OUTPUT\n\
  lurker keys --type gnupg\n\
  --passphrase             Ask for the shared passphrase\n\
  --key-file FILE          GnuPG passphrase file or OpenSSL shared AES key file\n\
  --public-key RECIPIENT   Encrypt to this recipient (repeatable)\n\
  --private-key FILE       Decrypt with an age or OpenSSL private key\n\
  --certificate FILE       Optional OpenSSL certificate matching --private-key\n\
  -F, --force              Replace an existing regular output file\n\n\
  age: use a passphrase or public keys.\n\
  GnuPG: use a passphrase or key file, public keys, or both credential types.\n\
  OpenSSL CMS: combine a passphrase, shared AES key file, and public certificates.\n\
  Encrypt or decrypt one file at a time.\n";

enum Action {
    File(FileRequest, bool),
    Keys,
    Help,
}

pub fn run_if_selected(args: &[OsString]) -> Option<i32> {
    let command = args.first()?.to_str()?;
    if !matches!(command, "encrypt" | "decrypt" | "keys") {
        return None;
    }
    let output = Output::new(ColorMode::Auto);
    let result = parse(command, &args[1..]).and_then(|action| match action {
        Action::File(request, ask_passphrase) => file_crypto::run(request, ask_passphrase),
        Action::Keys => {
            for key in file_crypto::list_gnupg_keys()? {
                output.raw_stdout(&format!("{key}\n"));
            }
            Ok(())
        }
        Action::Help => {
            output.raw_stdout(HELP);
            Ok(())
        }
    });
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            output.error(&error.message);
            error.exit_code
        }
    })
}

fn parse(command: &str, args: &[OsString]) -> AppResult<Action> {
    let mut args = args.iter();
    let mut backend = None;
    let mut public_keys = Vec::new();
    let mut private_key = None;
    let mut certificate = None;
    let mut key_file = None;
    let mut ask_passphrase = false;
    let mut force = false;
    let mut paths = Vec::new();

    while let Some(arg) = args.next() {
        let mut parts = arg.as_bytes().splitn(2, |byte| *byte == b'=');
        let option = parts.next().unwrap_or_default();
        let inline_value = parts.next().map(OsStr::from_bytes);
        let mut value = || {
            inline_value
                .or_else(|| args.next().map(OsString::as_os_str))
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    AppError::new(format!(
                        "{} needs a value.",
                        OsStr::from_bytes(option).to_string_lossy()
                    ))
                })
        };
        match option {
            b"-h" | b"--help" if inline_value.is_none() => return Ok(Action::Help),
            b"--" if inline_value.is_none() => {
                paths.extend(args.map(PathBuf::from));
                break;
            }
            b"--type" => {
                let selected = value()?
                    .to_str()
                    .and_then(FileBackend::parse)
                    .ok_or_else(|| AppError::new("Choose age, gnupg, or openssl for --type."))?;
                set_once(&mut backend, selected, "--type")?;
            }
            b"--key-file" => set_once(&mut key_file, PathBuf::from(value()?), "--key-file")?,
            b"--public-key" => public_keys.push(value()?.to_os_string()),
            b"--private-key" => {
                set_once(&mut private_key, PathBuf::from(value()?), "--private-key")?
            }
            b"--certificate" => {
                set_once(&mut certificate, PathBuf::from(value()?), "--certificate")?
            }
            b"--passphrase" if inline_value.is_none() && !ask_passphrase => ask_passphrase = true,
            b"-F" | b"--force" if inline_value.is_none() => force = true,
            _ if option.starts_with(b"-") => {
                return Err(AppError::new(format!(
                    "Invalid option {}.",
                    OsStr::from_bytes(option).to_string_lossy()
                )));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    let backend = backend.ok_or_else(|| AppError::new("Specify --type age, gnupg, or openssl."))?;
    if command == "keys" {
        if matches!(backend, FileBackend::GnuPg)
            && paths.is_empty()
            && !ask_passphrase
            && !force
            && key_file.is_none()
            && public_keys.is_empty()
            && private_key.is_none()
            && certificate.is_none()
        {
            return Ok(Action::Keys);
        }
        return Err(AppError::new("Usage: lurker keys --type gnupg"));
    }
    let [input, output] = <[PathBuf; 2]>::try_from(paths)
        .map_err(|_| AppError::new("Specify one input file and one output file."))?;
    let operation = if command == "encrypt" {
        FileOperation::Encrypt
    } else {
        FileOperation::Decrypt
    };
    Ok(Action::File(
        FileRequest {
            backend,
            operation,
            input,
            output,
            force,
            passphrase: None,
            key_file,
            public_keys,
            private_key,
            certificate,
        },
        ask_passphrase,
    ))
}

fn set_once<T>(slot: &mut Option<T>, value: T, option: &str) -> AppResult<()> {
    if slot.replace(value).is_some() {
        return Err(AppError::new(format!("Specify {option} only once.")));
    }
    Ok(())
}
