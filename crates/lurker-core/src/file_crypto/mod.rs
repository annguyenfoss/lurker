mod age_backend;
mod cms;
mod gpg;

use crate::{AppError, AppResult};
use std::ffi::OsString;
use std::fs::{self, Metadata};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum FileBackend {
    Age,
    GnuPg,
    OpenSsl,
}

impl FileBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "age" => Some(Self::Age),
            "gnupg" => Some(Self::GnuPg),
            "openssl" => Some(Self::OpenSsl),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Encrypt,
    Decrypt,
}

pub struct FileRequest {
    pub backend: FileBackend,
    pub operation: FileOperation,
    pub input: PathBuf,
    pub output: PathBuf,
    pub force: bool,
    pub passphrase: Option<String>,
    pub key_file: Option<PathBuf>,
    pub public_keys: Vec<OsString>,
    pub private_key: Option<PathBuf>,
    pub certificate: Option<PathBuf>,
}

pub fn list_gnupg_keys() -> AppResult<Vec<String>> {
    gpg::list_keys()
}

pub fn run(mut request: FileRequest, ask_passphrase: bool) -> AppResult<()> {
    validate_credentials(&request, ask_passphrase || request.passphrase.is_some())?;
    let input_metadata = fs::metadata(&request.input)
        .map_err(|error| AppError::io("Cannot inspect input file", error))?;
    if !input_metadata.is_file() {
        return Err(AppError::new("Input must be a regular file."));
    }
    request.input = fs::canonicalize(&request.input)
        .map_err(|error| AppError::io("Cannot resolve input file", error))?;

    let output_parent = request.output.parent().unwrap_or(Path::new("."));
    let output_parent = if output_parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        output_parent
    };
    let output_parent = fs::canonicalize(output_parent)
        .map_err(|error| AppError::io("Cannot resolve output directory", error))?;
    let output_name = request
        .output
        .file_name()
        .ok_or_else(|| AppError::new("Output must name a file."))?;
    let output = output_parent.join(output_name);
    validate_destination(&output, &input_metadata, request.force)?;

    if ask_passphrase {
        if request.passphrase.is_some() {
            return Err(AppError::new("Specify the passphrase only once."));
        }
        let passphrase = prompt_passphrase("Passphrase: ")?;
        if request.operation == FileOperation::Encrypt
            && passphrase != prompt_passphrase("Confirm passphrase: ")?
        {
            return Err(AppError::new("Passphrases do not match."));
        }
        request.passphrase = Some(passphrase);
    }
    if request
        .passphrase
        .as_deref()
        .is_some_and(|passphrase| passphrase.is_empty() || passphrase.contains(['\0', '\r', '\n']))
    {
        return Err(AppError::new(
            "Passphrase must be non-empty and contain no NUL or newline characters.",
        ));
    }

    let stage = tempfile::Builder::new()
        .prefix(".lurker-file-")
        .tempfile_in(&output_parent)
        .map_err(|error| AppError::io("Cannot stage output file", error))?;
    match request.backend {
        FileBackend::Age => age_backend::run(&request, stage.path())?,
        FileBackend::GnuPg => gpg::run(&request, stage.path())?,
        FileBackend::OpenSsl => cms::run(&request, stage.path())?,
    }
    stage
        .as_file()
        .sync_all()
        .map_err(|error| AppError::io("Cannot sync staged output", error))?;
    validate_destination(&output, &input_metadata, request.force)?;
    if request.force {
        stage.persist(&output)
    } else {
        stage.persist_noclobber(&output)
    }
    .map_err(|error| AppError::io("Cannot publish output file", error.error))?;
    Ok(())
}

fn validate_destination(path: &Path, input: &Metadata, force: bool) -> AppResult<()> {
    let destination = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(AppError::io("Cannot inspect output file", error)),
    };
    if fs::metadata(path)
        .is_ok_and(|other| other.dev() == input.dev() && other.ino() == input.ino())
    {
        return Err(AppError::new("Input and output refer to the same file."));
    }
    if !destination.is_file() || !force {
        return Err(AppError::new(
            "Output already exists. --force replaces an existing regular file.",
        ));
    }
    Ok(())
}

fn validate_credentials(request: &FileRequest, password: bool) -> AppResult<()> {
    use FileBackend::{Age, GnuPg, OpenSsl};
    use FileOperation::{Decrypt, Encrypt};

    let shared = request.key_file.is_some();
    let public = !request.public_keys.is_empty();
    let private = request.private_key.is_some();
    let certificate = request.certificate.is_some();
    let invalid_direction = match request.operation {
        Encrypt => private || certificate,
        Decrypt => public,
    };
    if invalid_direction {
        return Err(AppError::new(
            "Use --public-key for encryption and --private-key or --certificate for decryption.",
        ));
    }

    let valid = match (request.backend, request.operation) {
        (Age, Encrypt) => !shared && (password ^ public),
        (Age, Decrypt) => !shared && !certificate && (password ^ private),
        (GnuPg, Encrypt) => (password || shared || public) && !(password && shared),
        (GnuPg, Decrypt) => !private && !certificate && !(password && shared),
        (OpenSsl, Encrypt) => password || shared || public,
        (OpenSsl, Decrypt) => {
            usize::from(password) + usize::from(shared) + usize::from(private) == 1
                && (!certificate || private)
        }
    };
    if !valid {
        let message = match (request.backend, request.operation) {
            (Age, Encrypt) => "Choose --passphrase or one or more --public-key recipients for age.",
            (Age, Decrypt) => "Choose --passphrase or --private-key for age decryption.",
            (GnuPg, Encrypt) => "Choose --passphrase or --key-file, one or more --public-key recipients, or a combination of those.",
            (GnuPg, Decrypt) => "GnuPG finds the matching private key. You can also choose --passphrase or --key-file.",
            (OpenSsl, Encrypt) => "Choose --passphrase, --key-file, --public-key, or any combination.",
            (OpenSsl, Decrypt) => "Choose one of --passphrase, --key-file, or --private-key, with --certificate if needed.",
        };
        return Err(AppError::new(message));
    }

    for path in [
        &request.key_file,
        &request.private_key,
        &request.certificate,
    ]
    .into_iter()
    .flatten()
    {
        validate_credential_file(path)?;
    }
    if !matches!(request.backend, GnuPg) {
        for path in &request.public_keys {
            validate_credential_file(Path::new(path))?;
        }
    }
    Ok(())
}

fn validate_credential_file(path: &Path) -> AppResult<()> {
    let metadata = fs::metadata(path)
        .map_err(|error| AppError::io("Cannot inspect credential file", error))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(AppError::new(
            "Credential must be a non-empty regular file.",
        ));
    }
    Ok(())
}

pub(super) fn prompt_passphrase(label: &str) -> AppResult<String> {
    crate::credentials::prompt_passphrase(label)
        .map_err(|error| AppError::io("Cannot read passphrase from terminal", error))
}
