use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use age::armor::ArmoredReader;
use age::secrecy::SecretString;
use age::{Decryptor, Encryptor, Identity, Recipient};

use super::{prompt_passphrase, FileOperation, FileRequest};
use crate::{AppError, AppResult};

pub(super) fn run(request: &FileRequest, staged: &Path) -> AppResult<()> {
    let input = BufReader::new(
        File::open(&request.input).map_err(|err| AppError::io("Opening age input", err))?,
    );
    let output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(staged)
        .map_err(|err| AppError::io("Opening staged age output", err))?;

    match request.operation {
        FileOperation::Encrypt => encrypt(request, input, output),
        FileOperation::Decrypt => decrypt(request, input, output),
    }
}

fn encrypt(request: &FileRequest, mut input: BufReader<File>, output: File) -> AppResult<()> {
    let encryptor = if let Some(passphrase) = &request.passphrase {
        Encryptor::with_user_passphrase(SecretString::from(passphrase.clone()))
    } else {
        let mut recipients = Vec::new();
        for path in &request.public_keys {
            read_recipients(Path::new(path), &mut recipients)?;
        }
        Encryptor::with_recipients(recipients.iter().map(Box::as_ref))
            .map_err(|err| AppError::new(format!("Selecting age recipients: {err}")))?
    };

    let mut encrypted = encryptor
        .wrap_output(output)
        .map_err(|err| AppError::io("Starting age encryption", err))?;
    io::copy(&mut input, &mut encrypted)
        .map_err(|err| AppError::io("Encrypting file with age", err))?;
    encrypted
        .finish()
        .and_then(|mut file| file.flush())
        .map_err(|err| AppError::io("Finishing age encryption", err))
}

fn decrypt(request: &FileRequest, input: BufReader<File>, mut output: File) -> AppResult<()> {
    let decryptor = Decryptor::new_buffered(ArmoredReader::new(input.into_inner()))
        .map_err(|err| AppError::new(format!("Reading age file: {err}")))?;
    let identities: Vec<Box<dyn Identity>> = if decryptor.is_scrypt() {
        let passphrase = request
            .passphrase
            .as_ref()
            .ok_or_else(|| AppError::new("This age file requires --passphrase."))?;
        vec![Box::new(age::scrypt::Identity::new(SecretString::from(
            passphrase.clone(),
        ))) as Box<dyn Identity>]
    } else {
        let path = request
            .private_key
            .as_deref()
            .ok_or_else(|| AppError::new("This age file requires --private-key FILE."))?;
        read_identities(path)?
    };

    let mut plaintext = decryptor
        .decrypt(identities.iter().map(Box::as_ref))
        .map_err(|err| AppError::new(format!("Decrypting age file: {err}")))?;
    io::copy(&mut plaintext, &mut output)
        .and_then(|_| output.flush())
        .map_err(|err| AppError::io("Writing decrypted age file", err))
}

fn read_recipients(path: &Path, recipients: &mut Vec<Box<dyn Recipient>>) -> AppResult<()> {
    let input = File::open(path)
        .map_err(|err| AppError::io(format!("Opening age recipient {path:?}"), err))?;
    let initial_count = recipients.len();
    for (index, line) in BufReader::new(input).lines().enumerate() {
        let line =
            line.map_err(|err| AppError::io(format!("Reading age recipient {path:?}"), err))?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let recipient = parse_recipient(line).ok_or_else(|| {
            AppError::new(format!(
                "Invalid or unsupported age recipient in {path:?} on line {}.",
                index + 1,
            ))
        })?;
        recipients.push(recipient);
    }
    if recipients.len() == initial_count {
        return Err(AppError::new(format!("No age recipients in {path:?}.")));
    }
    Ok(())
}

fn parse_recipient(line: &str) -> Option<Box<dyn Recipient>> {
    if let Ok(recipient) = line.parse::<age::x25519::Recipient>() {
        return Some(Box::new(recipient));
    }
    if let Ok(recipient) = line.parse::<age::tag::Recipient>() {
        return Some(Box::new(recipient));
    }
    if let Ok(recipient) = line.parse::<age::tagpq::Recipient>() {
        return Some(Box::new(recipient));
    }
    line.parse::<age::ssh::Recipient>()
        .ok()
        .map(|recipient| Box::new(recipient) as Box<dyn Recipient>)
}

fn read_identities(path: &Path) -> AppResult<Vec<Box<dyn Identity>>> {
    let file = File::open(path)
        .map_err(|err| AppError::io(format!("Opening age identity {path:?}"), err))?;
    let mut input = BufReader::new(file);
    let prefix = input
        .fill_buf()
        .map_err(|err| AppError::io(format!("Reading age identity {path:?}"), err))?
        .trim_ascii_start();

    if prefix.starts_with(b"age-encryption.org/")
        || prefix.starts_with(b"-----BEGIN AGE ENCRYPTED FILE-----")
    {
        let identity = age::encrypted::Identity::from_buffer(
            ArmoredReader::new(input),
            None,
            IdentityPrompt(format!("Passphrase for age identity {path:?}")),
            None,
        )
        .map_err(|err| AppError::new(format!("Reading encrypted age identity {path:?}: {err}")))?
        .ok_or_else(|| {
            AppError::new(format!(
                "Age identity {path:?} requires passphrase protection."
            ))
        })?;
        return Ok(vec![Box::new(identity)]);
    }

    if prefix.starts_with(b"-----BEGIN OPENSSH PRIVATE KEY-----")
        || prefix.starts_with(b"-----BEGIN RSA PRIVATE KEY-----")
    {
        let identity = age::ssh::Identity::from_buffer(input, None)
            .map_err(|err| AppError::io(format!("Reading SSH private key {path:?}"), err))?;
        if matches!(identity, age::ssh::Identity::Unsupported(_)) {
            return Err(AppError::new(format!(
                "Unsupported SSH private key {path:?}."
            )));
        }
        return Ok(vec![Box::new(identity.with_callbacks(IdentityPrompt(
            format!("Passphrase for SSH private key {path:?}"),
        )))]);
    }

    let identities = age::IdentityFile::from_buffer(input)
        .map_err(|err| AppError::io(format!("Reading age identity {path:?}"), err))?
        .into_identities()
        .map_err(|err| AppError::new(format!("Loading age identity {path:?}: {err}")))?;
    if identities.is_empty() {
        return Err(AppError::new(format!("No age identities in {path:?}.")));
    }
    Ok(identities
        .into_iter()
        .map(|identity| identity as _)
        .collect())
}

#[derive(Clone)]
struct IdentityPrompt(String);

impl age::Callbacks for IdentityPrompt {
    fn display_message(&self, _message: &str) {}

    fn confirm(&self, _message: &str, _yes: &str, _no: Option<&str>) -> Option<bool> {
        None
    }

    fn request_public_string(&self, _description: &str) -> Option<String> {
        None
    }

    fn request_passphrase(&self, _description: &str) -> Option<SecretString> {
        prompt_passphrase(&self.0).ok().map(SecretString::from)
    }
}
