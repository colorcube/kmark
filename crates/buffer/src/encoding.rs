use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Supported file text encodings (strict decoding, lossless roundtrip).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextEncoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl TextEncoding {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 (BOM)",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1252 => "Windows-1252",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Utf8,
            Self::Utf8Bom,
            Self::Utf16Le,
            Self::Utf16Be,
            Self::Windows1252,
        ]
    }
}

/// Original line ending convention preserved on disk while normalizing in-memory buffers to `\n`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
}

impl LineEnding {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("Dateisystemfehler: {0}")]
    Fs(#[from] std::io::Error),
    #[error("Ungültige Zeichenkodierung für {0}")]
    Decode(String),
    #[error("Dokument enthält Zeichen, die in {0} nicht darstellbar sind")]
    UnrepresentableChar(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedDocument {
    pub text: String,
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

/// Windows-1252 mapping for `0x80..=0x9F`.
const CP1252_HIGH_CONTROL: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

/// Auto-detects BOM / strict UTF-8 / Windows-1252 fallback and normalizes newlines to `\n`.
pub fn decode_bytes_auto(bytes: &[u8]) -> Result<DecodedDocument, IoError> {
    let (raw, encoding) = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        let s = std::str::from_utf8(rest)
            .map_err(|_| IoError::Decode(TextEncoding::Utf8Bom.label().to_owned()))?;
        (s.to_owned(), TextEncoding::Utf8Bom)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        (decode_utf16(rest, true)?, TextEncoding::Utf16Le)
    } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        (decode_utf16(rest, false)?, TextEncoding::Utf16Be)
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        (s.to_owned(), TextEncoding::Utf8)
    } else {
        (decode_cp1252(bytes), TextEncoding::Windows1252)
    };

    let (text, line_ending) = normalize_newlines(&raw);
    Ok(DecodedDocument {
        text,
        encoding,
        line_ending,
    })
}

/// Decodes bytes with an explicitly chosen [`TextEncoding`].
pub fn decode_bytes_with(bytes: &[u8], encoding: TextEncoding) -> Result<DecodedDocument, IoError> {
    let raw = match encoding {
        TextEncoding::Utf8 => {
            let stripped = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            std::str::from_utf8(stripped)
                .map_err(|_| IoError::Decode(encoding.label().to_owned()))?
                .to_owned()
        }
        TextEncoding::Utf8Bom => {
            let stripped = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
            std::str::from_utf8(stripped)
                .map_err(|_| IoError::Decode(encoding.label().to_owned()))?
                .to_owned()
        }
        TextEncoding::Utf16Le => {
            let stripped = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
            decode_utf16(stripped, true)?
        }
        TextEncoding::Utf16Be => {
            let stripped = bytes.strip_prefix(&[0xFE, 0xFF]).unwrap_or(bytes);
            decode_utf16(stripped, false)?
        }
        TextEncoding::Windows1252 => decode_cp1252(bytes),
    };
    let (text, line_ending) = normalize_newlines(&raw);
    Ok(DecodedDocument {
        text,
        encoding,
        line_ending,
    })
}

/// Encodes normalized `\n` text into `encoding` and `line_ending`.
pub fn encode_text(
    text: &str,
    encoding: TextEncoding,
    line_ending: LineEnding,
) -> Result<Vec<u8>, IoError> {
    let converted = match line_ending {
        LineEnding::Lf => None,
        LineEnding::CrLf => Some(text.replace('\n', "\r\n")),
    };
    let body = converted.as_deref().unwrap_or(text);

    match encoding {
        TextEncoding::Utf8 => Ok(body.as_bytes().to_vec()),
        TextEncoding::Utf8Bom => {
            let mut out = Vec::with_capacity(body.len().saturating_add(3));
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            out.extend_from_slice(body.as_bytes());
            Ok(out)
        }
        TextEncoding::Utf16Le => {
            let mut out = Vec::with_capacity(body.len().saturating_mul(2).saturating_add(2));
            out.extend_from_slice(&[0xFF, 0xFE]);
            for unit in body.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            Ok(out)
        }
        TextEncoding::Utf16Be => {
            let mut out = Vec::with_capacity(body.len().saturating_mul(2).saturating_add(2));
            out.extend_from_slice(&[0xFE, 0xFF]);
            for unit in body.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            Ok(out)
        }
        TextEncoding::Windows1252 => encode_cp1252(body),
    }
}

/// Reads and decodes a file from `path`.
pub fn read_file_auto(path: &Path) -> Result<DecodedDocument, IoError> {
    let bytes = fs::read(path)?;
    decode_bytes_auto(&bytes)
}

/// Maximum symbolic link hops followed when resolving a save target.
const MAX_SYMLINK_DEPTH: usize = 40;

/// Writes `text` atomically to `path` via a synced sibling temp file so the original file
/// is never truncated or corrupted on error.
///
/// - Follows symbolic links (`canonicalize` / `read_link`) so saving through a symlink updates
///   the target file without replacing the symlink itself.
/// - Preserves existing file permissions (`mode`) when overwriting an existing file, or applies
///   standard `0666 & !umask` permissions when creating a new file (avoiding `0600` temp modes).
/// - Calls `sync_all()` on both the temporary file and the parent directory after rename.
pub fn write_file_atomic(
    path: &Path,
    text: &str,
    encoding: TextEncoding,
    line_ending: LineEnding,
) -> Result<(), IoError> {
    let encoded = encode_text(text, encoding, line_ending)?;
    let resolved_path = resolve_save_target(path)?;
    let parent = resolved_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let existing_perms = fs::metadata(&resolved_path).ok().map(|m| m.permissions());

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(&encoded)?;
    if let Some(perms) = existing_perms {
        fs::set_permissions(tmp.path(), perms)?;
    } else {
        apply_default_new_file_permissions(tmp.path())?;
    }
    tmp.as_file().sync_all()?;
    tmp.persist(&resolved_path)
        .map_err(|e| IoError::Fs(e.error))?;
    sync_directory(parent);
    Ok(())
}

fn resolve_save_target(path: &Path) -> Result<PathBuf, std::io::Error> {
    if let Ok(canon) = fs::canonicalize(path) {
        return Ok(canon);
    }

    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_DEPTH {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = fs::read_link(&current)?;
                current = if target.is_absolute() {
                    target
                } else {
                    current
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .join(target)
                };
                if let Ok(canon) = fs::canonicalize(&current) {
                    return Ok(canon);
                }
            }
            Ok(_) => return Ok(current),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(current),
            Err(err) => return Err(err),
        }
    }

    Err(std::io::Error::other("too many levels of symbolic links"))
}

#[cfg(unix)]
fn apply_default_new_file_permissions(path: &Path) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;

    let umask = read_process_umask().unwrap_or(0o022);
    let mode = 0o666 & !umask;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn apply_default_new_file_permissions(path: &Path) -> Result<(), std::io::Error> {
    let _ = path;
    Ok(())
}

#[cfg(unix)]
fn read_process_umask() -> Option<u32> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Umask:") {
            return u32::from_str_radix(rest.trim(), 8).ok();
        }
    }
    None
}

fn sync_directory(dir: &Path) {
    if let Ok(dir_file) = fs::File::open(dir) {
        let _ = dir_file.sync_all();
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, IoError> {
    if !bytes.len().is_multiple_of(2) {
        return Err(IoError::Decode("UTF-16 (ungerade Byteanzahl)".to_owned()));
    }
    let mut units = Vec::with_capacity(bytes.len() / 2);
    let (chunks, _) = bytes.as_chunks::<2>();
    for &[b0, b1] in chunks {
        let u = if little_endian {
            u16::from_le_bytes([b0, b1])
        } else {
            u16::from_be_bytes([b0, b1])
        };
        units.push(u);
    }
    String::from_utf16(&units).map_err(|_| IoError::Decode("UTF-16".to_owned()))
}

fn decode_cp1252(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let ch = if (0x80..=0x9F).contains(&b) {
            CP1252_HIGH_CONTROL
                .get(usize::from(b.saturating_sub(0x80)))
                .copied()
                .unwrap_or('\u{FFFD}')
        } else {
            char::from(b)
        };
        out.push(ch);
    }
    out
}

fn encode_cp1252(text: &str) -> Result<Vec<u8>, IoError> {
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let code = u32::from(ch);
        if code <= 0x7F || (0xA0..=0xFF).contains(&code) {
            out.push(code as u8);
        } else if let Some(idx) = CP1252_HIGH_CONTROL.iter().position(|&c| c == ch) {
            out.push((idx as u8).saturating_add(0x80));
        } else {
            return Err(IoError::UnrepresentableChar(
                TextEncoding::Windows1252.label().to_owned(),
            ));
        }
    }
    Ok(out)
}

fn normalize_newlines(raw: &str) -> (String, LineEnding) {
    let has_crlf = raw.contains("\r\n");
    let has_cr = raw.contains('\r');
    let ending = if has_crlf {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    };
    if !has_cr {
        return (raw.to_owned(), ending);
    }
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    (normalized, ending)
}
