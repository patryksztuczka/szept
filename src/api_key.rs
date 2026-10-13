//! Where the Soniox API key comes from: the `SONIOX_API_KEY` environment
//! variable when set, otherwise a file in the data folder readable only by
//! the current user.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{Result, WrapErr, eyre};

pub const ENV_VAR: &str = "SONIOX_API_KEY";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Environment,
    File,
}

pub fn path() -> Result<PathBuf> {
    Ok(crate::app_paths::support_dir()?.join("soniox-api-key"))
}

pub fn load() -> Result<Option<(String, KeySource)>> {
    if let Some(key) = std::env::var(ENV_VAR).ok().and_then(|key| clean(&key)) {
        return Ok(Some((key, KeySource::Environment)));
    }
    Ok(load_from(&path()?)?.map(|key| (key, KeySource::File)))
}

pub fn save(key: &str) -> Result<String> {
    save_to(&path()?, key)
}

pub fn remove() -> Result<()> {
    match fs::remove_file(path()?) {
        Err(error) if error.kind() != ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// Shows enough of a key to recognise it, e.g. `abcd…wxyz`.
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len());
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// A key is one token of visible ASCII; surrounding whitespace is dropped.
fn clean(key: &str) -> Option<String> {
    let key = key.trim();
    (!key.is_empty() && key.chars().all(|c| c.is_ascii_graphic())).then(|| key.to_owned())
}

fn load_from(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(clean(&contents)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).wrap_err_with(|| format!("could not read {}", path.display())),
    }
}

fn save_to(path: &Path, key: &str) -> Result<String> {
    let key = clean(key).ok_or_else(|| eyre!("the clipboard does not hold an API key"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let _ = fs::remove_file(&temporary);
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?
        .write_all(key.as_bytes())?;
    fs::rename(&temporary, path)
        .wrap_err_with(|| format!("could not replace {}", path.display()))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn saves_trimmed_key_readable_only_by_owner() {
        let dir = std::env::temp_dir().join(format!("szept-key-{}", std::process::id()));
        let path = dir.join("soniox-api-key");
        assert_eq!(load_from(&path).unwrap(), None);
        assert_eq!(save_to(&path, "  abc123\n").unwrap(), "abc123");
        assert_eq!(load_from(&path).unwrap().as_deref(), Some("abc123"));
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_text_that_is_not_a_key() {
        assert_eq!(clean(""), None);
        assert_eq!(clean("two words"), None);
        assert_eq!(clean(" key\n").as_deref(), Some("key"));
    }

    #[test]
    fn masks_the_middle_of_a_key() {
        assert_eq!(mask("abcdefghijklmnop"), "abcd…mnop");
        assert_eq!(mask("short"), "•••••");
    }
}
