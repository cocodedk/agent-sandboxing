//! Append JSON lines to the log; gzip and rotate it when it would pass the size cap.

use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt, chown};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::constants::{
    LINE_END_BYTE, LOG_DIR_MODE, LOG_FILE_MODE, LOG_GROUP, ROOT_UID, ROTATED_SUFFIX,
};
use crate::record::Record;

/// The uid and gid the log files get: root and the log group when running as root, else none.
pub fn log_owner(euid: u32) -> io::Result<Option<(u32, u32)>> {
    if euid != ROOT_UID {
        return Ok(None);
    }
    let name = CString::new(LOG_GROUP).map_err(io::Error::other)?;
    // SAFETY: getgrnam takes a valid C string; its result is read at once, before any other call.
    let group = unsafe { libc::getgrnam(name.as_ptr()) };
    if group.is_null() {
        return Err(io::Error::other(format!("no group {LOG_GROUP}")));
    }
    Ok(Some((ROOT_UID, unsafe { (*group).gr_gid })))
}

/// The effective uid of this process.
pub fn euid() -> u32 {
    // SAFETY: geteuid cannot fail and touches no memory.
    unsafe { libc::geteuid() }
}

/// A missing file is no error: rotation removes and renames copies that may not exist yet.
fn unless_missing(result: io::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Set the mode, and the owner when one is given.
fn own(path: &Path, mode: u32, owner: Option<(u32, u32)>) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    match owner {
        Some((uid, gid)) => chown(path, Some(uid), Some(gid)),
        None => Ok(()),
    }
}

/// One log file plus up to keep gzip copies: name.1.gz is the newest.
pub struct Writer {
    path: PathBuf,
    max_bytes: u64,
    keep: u32,
    owner: Option<(u32, u32)>,
    file: File,
    size: u64,
}

impl Writer {
    pub fn new(
        path: &Path,
        max_bytes: u64,
        keep: u32,
        owner: Option<(u32, u32)>,
    ) -> io::Result<Self> {
        let dir = path
            .parent()
            .ok_or_else(|| io::Error::other("the log path has no folder"))?;
        fs::create_dir_all(dir)?;
        own(dir, LOG_DIR_MODE, owner)?;
        let file = Self::open(path, owner)?;
        let size = file.metadata()?.len();
        Ok(Writer {
            path: path.to_path_buf(),
            max_bytes,
            keep,
            owner,
            file,
            size,
        })
    }

    fn open(path: &Path, owner: Option<(u32, u32)>) -> io::Result<File> {
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(LOG_FILE_MODE)
            .open(path)?;
        own(path, LOG_FILE_MODE, owner)?;
        Ok(file)
    }

    /// The path of the number-th newest gzip copy.
    fn rotated(&self, number: u32) -> PathBuf {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(format!(".{number}{ROTATED_SUFFIX}"));
        self.path.with_file_name(name)
    }

    /// Append one record as a JSON line, rotating first if it would pass the cap.
    pub fn write(&mut self, record: &Record) -> io::Result<()> {
        let mut line = serde_json::to_vec(record)?;
        line.push(LINE_END_BYTE);
        let length = line.len() as u64;
        if self.size > 0 && self.size + length > self.max_bytes {
            self.rotate()?;
        }
        self.file.write_all(&line)?;
        self.size += length;
        Ok(())
    }

    /// Gzip the current file to .1.gz, shift older copies up, drop those past keep.
    fn rotate(&mut self) -> io::Result<()> {
        unless_missing(fs::remove_file(self.rotated(self.keep)))?;
        for number in (1..self.keep).rev() {
            unless_missing(fs::rename(self.rotated(number), self.rotated(number + 1)))?;
        }
        let newest = self.rotated(1);
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(LOG_FILE_MODE)
            .open(&newest)?;
        let mut target = GzEncoder::new(file, Compression::default());
        io::copy(&mut File::open(&self.path)?, &mut target)?;
        target.finish()?;
        own(&newest, LOG_FILE_MODE, self.owner)?;
        fs::remove_file(&self.path)?;
        self.file = Self::open(&self.path, self.owner)?;
        self.size = 0;
        Ok(())
    }
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
