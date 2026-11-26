//! A small binding to the installed OpenSSL CMS API. This lets the symmetric
//! recipients use the same file format as the CLI without placing secrets in argv.

use super::{backend_path, openssl_version, FileOperation, FileRequest, Openssl};
use crate::{AppError, AppResult};
use libloading::Library;
use std::collections::HashSet;
use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CStr};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::{self, NonNull};
use std::sync::OnceLock;

const CMS_BINARY: c_uint = 0x80;
const CMS_STREAM: c_uint = 0x1000;
const CMS_PARTIAL: c_uint = 0x4000;
const BIO_NOCLOSE: c_int = 0;
const AES_KEY_BYTES: usize = 32;
// OpenSSL's CMS PWRI parameters use PBKDF2-HMAC-SHA1 with RFC 3211 wrapping.
const PASSWORD_ITERATIONS: c_int = 1_400_000;

// All of these structures are opaque OpenSSL allocations. The only operations
// on their addresses are calls to the public API loaded below.
enum Bio {}
enum Cms {}
enum Cipher {}
enum Certificate {}
enum Recipient {}

macro_rules! openssl_api {
    ($( $field:ident : $name:literal => $ty:ty ),* $(,)?) => {
        struct Api { $( $field: $ty, )* }

        impl Api {
            fn load(library: &Library) -> Result<Self, libloading::Error> {
                // Each signature is from the public OpenSSL headers. The
                // library is retained for the lifetime of this function table.
                Ok(Self { $( $field: *unsafe { library.get(concat!($name, "\0").as_bytes())? }, )* })
            }
        }
    };
}

openssl_api! {
    bio_fd: "BIO_new_fd" => unsafe extern "C" fn(c_int, c_int) -> *mut Bio,
    bio_free: "BIO_free_all" => unsafe extern "C" fn(*mut Bio),
    cms_free: "CMS_ContentInfo_free" => unsafe extern "C" fn(*mut Cms),
    cms_encrypt: "CMS_encrypt" => unsafe extern "C" fn(*const c_void, *mut Bio, *const Cipher, c_uint) -> *mut Cms,
    cms_decrypt: "CMS_decrypt" => unsafe extern "C" fn(*mut Cms, *mut c_void, *mut Certificate, *mut Bio, *mut Bio, c_uint) -> c_int,
    cms_read: "d2i_CMS_bio" => unsafe extern "C" fn(*mut Bio, *mut *mut Cms) -> *mut Cms,
    cms_write: "i2d_CMS_bio_stream" => unsafe extern "C" fn(*mut Bio, *mut Cms, *mut Bio, c_int) -> c_int,
    add_certificate: "CMS_add1_recipient_cert" => unsafe extern "C" fn(*mut Cms, *mut Certificate, c_uint) -> *mut Recipient,
    add_key: "CMS_add0_recipient_key" => unsafe extern "C" fn(*mut Cms, c_int, *mut u8, usize, *mut u8, usize, *mut c_void, *mut c_void, *mut c_void) -> *mut Recipient,
    add_password: "CMS_add0_recipient_password" => unsafe extern "C" fn(*mut Cms, c_int, c_int, c_int, *mut u8, isize, *const Cipher) -> *mut Recipient,
    select_key: "CMS_decrypt_set1_key" => unsafe extern "C" fn(*mut Cms, *mut u8, usize, *const u8, usize) -> c_int,
    select_password: "CMS_decrypt_set1_password" => unsafe extern "C" fn(*mut Cms, *mut u8, isize) -> c_int,
    aes_gcm: "EVP_aes_256_gcm" => unsafe extern "C" fn() -> *const Cipher,
    aes_cbc: "EVP_aes_256_cbc" => unsafe extern "C" fn() -> *const Cipher,
    read_pem_certificate: "PEM_read_bio_X509" => unsafe extern "C" fn(*mut Bio, *mut *mut Certificate, Option<unsafe extern "C" fn(*mut c_char, c_int, c_int, *mut c_void) -> c_int>, *mut c_void) -> *mut Certificate,
    read_der_certificate: "d2i_X509_bio" => unsafe extern "C" fn(*mut Bio, *mut *mut Certificate) -> *mut Certificate,
    certificate_free: "X509_free" => unsafe extern "C" fn(*mut Certificate),
    allocate: "CRYPTO_malloc" => unsafe extern "C" fn(usize, *const c_char, c_int) -> *mut c_void,
    clear_free: "CRYPTO_clear_free" => unsafe extern "C" fn(*mut c_void, usize, *const c_char, c_int),
    clear_errors: "ERR_clear_error" => unsafe extern "C" fn(),
    last_error: "ERR_peek_last_error" => unsafe extern "C" fn() -> c_ulong,
    error_string: "ERR_error_string_n" => unsafe extern "C" fn(c_ulong, *mut c_char, usize),
}

struct Runtime {
    api: Api,
    _library: Library,
    version: String,
}

// Retain the library while OpenSSL's own shutdown handlers can still run.
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn runtime(openssl: &Openssl) -> AppResult<&'static Api> {
    if RUNTIME.get().is_none() {
        let mut checked = HashSet::new();
        for candidate in openssl.library_candidates() {
            let Some(path) = backend_path(&candidate).filter(|path| checked.insert(path.clone()))
            else {
                continue;
            };
            if let Some(runtime) = Runtime::load(&path, &openssl.version) {
                let _ = RUNTIME.set(runtime);
                break;
            }
        }
    }
    RUNTIME
        .get()
        .filter(|runtime| runtime.version == openssl.version)
        .map(|runtime| &runtime.api)
        .ok_or_else(|| {
            AppError::new(format!(
                "Could not find a libcrypto CMS library matching OpenSSL {} at {}.",
                openssl.version,
                openssl.executable.display()
            ))
        })
}

impl Runtime {
    fn load(path: &Path, expected_version: &str) -> Option<Self> {
        // `backend_path` has already resolved the absolute installation path
        // and checked its ownership for elevated callers.
        let library = unsafe { Library::new(path) }.ok()?;
        let version_fn = unsafe {
            library.get::<unsafe extern "C" fn(c_int) -> *const c_char>(b"OpenSSL_version\0")
        }
        .ok()?;
        let version = unsafe { version_fn(0) };
        if version.is_null() {
            return None;
        }
        let version = openssl_version(unsafe { CStr::from_ptr(version) }.to_str().ok()?)?;
        if version != expected_version {
            return None;
        }
        let api = Api::load(&library).ok()?;
        Some(Self {
            api,
            _library: library,
            version,
        })
    }
}

pub(super) fn run(openssl: &Openssl, request: &FileRequest, staged: &Path) -> AppResult<()> {
    let api = runtime(openssl)?;
    unsafe { (api.clear_errors)() };
    let input =
        File::open(&request.input).map_err(|error| AppError::io("Open CMS input", error))?;
    let output = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(staged)
        .map_err(|error| AppError::io("Open staged CMS output", error))?;
    let input_bio = api.fd(&input)?;
    let output_bio = api.fd(&output)?;

    match request.operation {
        FileOperation::Encrypt => api.encrypt(request, input_bio.as_ptr(), output_bio.as_ptr()),
        FileOperation::Decrypt => api.decrypt(request, input_bio.as_ptr(), output_bio.as_ptr()),
    }
}

impl Api {
    fn encrypt(&self, request: &FileRequest, input: *mut Bio, output: *mut Bio) -> AppResult<()> {
        let flags = CMS_BINARY | CMS_STREAM;
        let cipher = unsafe { (self.aes_gcm)() };
        let pointer = unsafe {
            (self.cms_encrypt)(ptr::null(), ptr::null_mut(), cipher, flags | CMS_PARTIAL)
        };
        let cms = self.owned(pointer, self.cms_free, "Create AES-256-GCM CMS envelope")?;

        for path in &request.public_keys {
            let path = Path::new(path);
            let certificate = self.certificate(path)?;
            if unsafe { (self.add_certificate)(cms.as_ptr(), certificate.as_ptr(), 0) }.is_null() {
                return Err(
                    self.error(&format!("Add CMS recipient certificate {}", path.display()))
                );
            }
        }

        if let Some(path) = &request.key_file {
            let key = self.read_key(path)?;
            let id = SecretBytes::copy(self, &[1])?;
            // A zero algorithm NID selects AES wrap from the supplied key size.
            let recipient = unsafe {
                (self.add_key)(
                    cms.as_ptr(),
                    0,
                    key.as_ptr(),
                    key.len,
                    id.as_ptr(),
                    id.len,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            };
            if recipient.is_null() {
                return Err(self.error("Add shared AES recipient"));
            }
            // Ownership of both OpenSSL allocations transfers on success.
            std::mem::forget(key);
            std::mem::forget(id);
        }

        if let Some(passphrase) = &request.passphrase {
            let passphrase = SecretBytes::copy(self, passphrase.as_bytes())?;
            // CBC wraps only the generated file key for this PWRI entry. The
            // file contents still use the envelope's authenticated AES-GCM.
            let wrap_cipher = unsafe { (self.aes_cbc)() };
            let recipient = unsafe {
                (self.add_password)(
                    cms.as_ptr(),
                    PASSWORD_ITERATIONS,
                    0,
                    0,
                    passphrase.as_ptr(),
                    passphrase.len as isize,
                    wrap_cipher,
                )
            };
            if recipient.is_null() {
                return Err(self.error("Add CMS passphrase recipient"));
            }
            std::mem::forget(passphrase);
        }

        self.require(
            unsafe { (self.cms_write)(output, cms.as_ptr(), input, flags as c_int) },
            "Encrypt CMS file",
        )
    }

    fn decrypt(&self, request: &FileRequest, input: *mut Bio, output: *mut Bio) -> AppResult<()> {
        let pointer = unsafe { (self.cms_read)(input, ptr::null_mut()) };
        let cms = self.owned(pointer, self.cms_free, "Read binary CMS envelope")?;
        if let Some(path) = &request.key_file {
            let key = self.read_key(path)?;
            // A null identifier lets OpenSSL try this key against each KEK
            // recipient, including envelopes created outside Lurker.
            let result =
                unsafe { (self.select_key)(cms.as_ptr(), key.as_ptr(), key.len, ptr::null(), 0) };
            self.require(result, "Unlock CMS file with shared AES key")?;
        } else if let Some(passphrase) = &request.passphrase {
            let passphrase = SecretBytes::copy(self, passphrase.as_bytes())?;
            let result = unsafe {
                (self.select_password)(cms.as_ptr(), passphrase.as_ptr(), passphrase.len as isize)
            };
            self.require(result, "Unlock CMS file with passphrase")?;
        }
        self.require(
            unsafe {
                (self.cms_decrypt)(
                    cms.as_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    output,
                    CMS_BINARY,
                )
            },
            "Decrypt CMS file",
        )
    }

    fn read_key(&self, path: &Path) -> AppResult<SecretBytes<'_>> {
        let mut file =
            File::open(path).map_err(|error| AppError::io("Open CMS AES keyfile", error))?;
        let mut buffer = SecretBytes::new(self, AES_KEY_BYTES * 2 + 3)?;
        let bytes = buffer.as_mut_slice();
        let mut len = 0;
        while len < bytes.len() {
            let count = file
                .read(&mut bytes[len..])
                .map_err(|error| AppError::io("Read CMS AES keyfile", error))?;
            if count == 0 {
                break;
            }
            len += count;
        }
        if len == AES_KEY_BYTES {
            return SecretBytes::copy(self, &bytes[..len]);
        }

        let data = &bytes[..len];
        let hex = data
            .strip_suffix(b"\r\n")
            .or_else(|| data.strip_suffix(b"\n"))
            .unwrap_or(data);
        let error =
            || AppError::new("A CMS AES keyfile must contain 32 raw bytes or 64 hex digits.");
        if hex.len() != AES_KEY_BYTES * 2 {
            return Err(error());
        }
        let mut key = SecretBytes::new(self, AES_KEY_BYTES)?;
        for (pair, octet) in hex.chunks_exact(2).zip(key.as_mut_slice()) {
            let high = char::from(pair[0]).to_digit(16).ok_or_else(error)?;
            let low = char::from(pair[1]).to_digit(16).ok_or_else(error)?;
            *octet = ((high << 4) | low) as u8;
        }
        Ok(key)
    }

    fn certificate(&self, path: &Path) -> AppResult<Owned<Certificate>> {
        let mut file = File::open(path).map_err(|error| {
            AppError::io(
                format!("Open recipient certificate {}", path.display()),
                error,
            )
        })?;
        let bio = self.fd(&file)?;
        let mut certificate = unsafe {
            (self.read_pem_certificate)(bio.as_ptr(), ptr::null_mut(), None, ptr::null_mut())
        };
        if certificate.is_null() {
            unsafe { (self.clear_errors)() };
            file.rewind()
                .map_err(|error| AppError::io("Rewind recipient certificate", error))?;
            certificate = unsafe { (self.read_der_certificate)(bio.as_ptr(), ptr::null_mut()) };
        }
        self.owned(
            certificate,
            self.certificate_free,
            &format!("Read X.509 certificate {}", path.display()),
        )
    }

    fn fd(&self, file: &File) -> AppResult<Owned<Bio>> {
        let bio = unsafe { (self.bio_fd)(file.as_raw_fd(), BIO_NOCLOSE) };
        self.owned(bio, self.bio_free, "Attach file to OpenSSL CMS")
    }

    fn owned<T>(
        &self,
        pointer: *mut T,
        free: unsafe extern "C" fn(*mut T),
        action: &str,
    ) -> AppResult<Owned<T>> {
        NonNull::new(pointer)
            .map(|pointer| Owned { pointer, free })
            .ok_or_else(|| self.error(action))
    }

    fn require(&self, result: c_int, action: &str) -> AppResult<()> {
        if result == 1 {
            Ok(())
        } else {
            Err(self.error(action))
        }
    }

    fn error(&self, action: &str) -> AppError {
        let error = unsafe { (self.last_error)() };
        if error == 0 {
            return AppError::new(format!("{action} failed."));
        }
        let mut message = [0; 256];
        unsafe { (self.error_string)(error, message.as_mut_ptr(), message.len()) };
        let message = unsafe { CStr::from_ptr(message.as_ptr()) }.to_string_lossy();
        AppError::new(format!("{action} failed ({message})."))
    }
}

struct Owned<T> {
    pointer: NonNull<T>,
    free: unsafe extern "C" fn(*mut T),
}

impl<T> Owned<T> {
    fn as_ptr(&self) -> *mut T {
        self.pointer.as_ptr()
    }
}

impl<T> Drop for Owned<T> {
    fn drop(&mut self) {
        unsafe { (self.free)(self.as_ptr()) };
    }
}

struct SecretBytes<'a> {
    api: &'a Api,
    pointer: NonNull<u8>,
    len: usize,
}

impl<'a> SecretBytes<'a> {
    fn new(api: &'a Api, len: usize) -> AppResult<Self> {
        let pointer = unsafe { (api.allocate)(len, ptr::null(), 0) };
        let pointer = NonNull::new(pointer.cast::<u8>())
            .ok_or_else(|| api.error("Allocate CMS credential"))?;
        // The storage is initialized before any Rust slice borrows it.
        unsafe { pointer.as_ptr().write_bytes(0, len) };
        Ok(Self { api, pointer, len })
    }

    fn copy(api: &'a Api, bytes: &[u8]) -> AppResult<Self> {
        let mut secret = Self::new(api, bytes.len())?;
        secret.as_mut_slice().copy_from_slice(bytes);
        Ok(secret)
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.as_ptr(), self.len) }
    }

    fn as_ptr(&self) -> *mut u8 {
        self.pointer.as_ptr()
    }
}

impl Drop for SecretBytes<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.clear_free)(self.as_ptr().cast(), self.len, ptr::null(), 0) };
    }
}
