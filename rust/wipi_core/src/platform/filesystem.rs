// wie_cli/src/filesystem.rs를 base 경로만 주입받도록 옮긴 것.
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

use wie_backend::Filesystem;

pub struct FsFilesystem {
    base_path: PathBuf,
}

impl FsFilesystem {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn path_for(&self, aid: &str, path: &str) -> Option<PathBuf> {
        let sanitized_aid: String = aid.chars().filter(|c| !matches!(c, '/' | '\\' | '\0')).collect();
        if sanitized_aid.is_empty() || sanitized_aid == "." || sanitized_aid == ".." {
            tracing::error!(aid, path, "rejected: invalid aid");
            return None;
        }

        let mut normalized = PathBuf::new();
        for component in Path::new(path).components() {
            match component {
                Component::Normal(c) => normalized.push(c),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    tracing::error!(aid, path, "path traversal attempt rejected");
                    return None;
                }
            }
        }

        if normalized.as_os_str().is_empty() {
            tracing::error!(aid, path, "rejected: empty normalized path");
            return None;
        }

        Some(super::storage::namespace_dir(&self.base_path, &sanitized_aid).join(normalized))
    }
}

#[async_trait::async_trait]
impl Filesystem for FsFilesystem {
    async fn remove(&self, aid: &str, path: &str) -> bool {
        let Some(disk_path) = self.path_for(aid, path) else {
            return false;
        };

        fs::remove_file(&disk_path).is_ok()
    }

    async fn usage(&self, aid: &str) -> u64 {
        fn directory_usage(path: &Path) -> u64 {
            let Ok(entries) = fs::read_dir(path) else {
                return 0;
            };
            entries
                .filter_map(Result::ok)
                .map(|entry| match entry.file_type() {
                    Ok(file_type) if file_type.is_file() => entry.metadata().map(|metadata| metadata.len()).unwrap_or(0),
                    Ok(file_type) if file_type.is_dir() => directory_usage(&entry.path()),
                    _ => 0,
                })
                .sum()
        }

        let sanitized_aid: String = aid.chars().filter(|c| !matches!(c, '/' | '\\' | '\0')).collect();
        if sanitized_aid.is_empty() || sanitized_aid == "." || sanitized_aid == ".." {
            return 0;
        }
        directory_usage(&super::storage::namespace_dir(&self.base_path, &sanitized_aid))
    }

    async fn exists(&self, aid: &str, path: &str) -> bool {
        let Some(disk_path) = self.path_for(aid, path) else {
            return false;
        };

        match disk_path.metadata() {
            Ok(md) => md.is_file(),
            Err(_) => false,
        }
    }

    async fn size(&self, aid: &str, path: &str) -> Option<usize> {
        let disk_path = self.path_for(aid, path)?;
        let md = disk_path.metadata().ok()?;
        if !md.is_file() {
            return None;
        }
        Some(md.len() as usize)
    }

    async fn read(&self, aid: &str, path: &str, offset: usize, count: usize, buf: &mut [u8]) -> Option<usize> {
        let disk_path = self.path_for(aid, path)?;

        let mut file = match OpenOptions::new().read(true).open(&disk_path) {
            Ok(f) => f,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    return None;
                }
                tracing::warn!(aid, path, error = %err, "read: open failed");
                return None;
            }
        };

        let size = file.metadata().map(|m| m.len() as usize).unwrap_or(0);
        if offset >= size {
            return Some(0);
        }

        if let Err(err) = file.seek(SeekFrom::Start(offset as u64)) {
            tracing::warn!(aid, path, error = %err, "read: seek failed");
            return Some(0);
        }

        let to_read = core::cmp::min(count, size - offset);
        let slice = &mut buf[..to_read];
        match file.read_exact(slice) {
            Ok(()) => Some(to_read),
            Err(err) => {
                tracing::warn!(aid, path, error = %err, "read: IO error");
                Some(0)
            }
        }
    }

    async fn write(&self, aid: &str, path: &str, offset: usize, data: &[u8]) -> usize {
        let Some(disk_path) = self.path_for(aid, path) else {
            return 0;
        };

        if let Some(parent) = disk_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            tracing::warn!(aid, path, error = %err, "write: create parent dir failed");
            return 0;
        }

        let mut file = match OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&disk_path) {
            Ok(f) => f,
            Err(err) => {
                tracing::warn!(aid, path, error = %err, "write: open failed");
                return 0;
            }
        };

        let current_size = file.metadata().map(|m| m.len() as usize).unwrap_or(0);

        if offset > current_size
            && let Err(err) = file.set_len(offset as u64)
        {
            tracing::warn!(aid, path, error = %err, "write: set_len extend failed");
            return 0;
        }

        if let Err(err) = file.seek(SeekFrom::Start(offset as u64)) {
            tracing::warn!(aid, path, error = %err, "write: seek failed");
            return 0;
        }

        match file.write_all(data) {
            Ok(()) => data.len(),
            Err(err) => {
                tracing::warn!(aid, path, error = %err, "write: write_all failed");
                0
            }
        }
    }

    async fn truncate(&self, aid: &str, path: &str, len: usize) {
        let Some(disk_path) = self.path_for(aid, path) else {
            return;
        };

        if let Some(parent) = disk_path.parent()
            && let Err(err) = fs::create_dir_all(parent)
        {
            tracing::warn!(aid, path, error = %err, "truncate: create parent dir failed");
            return;
        }

        let file = match OpenOptions::new().read(true).write(true).create(true).truncate(false).open(&disk_path) {
            Ok(f) => f,
            Err(err) => {
                tracing::warn!(aid, path, error = %err, "truncate: open failed");
                return;
            }
        };

        if let Err(err) = file.set_len(len as u64) {
            tracing::warn!(aid, path, error = %err, "truncate: set_len failed");
        }
    }
}
