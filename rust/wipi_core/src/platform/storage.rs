use std::{fs, path::{Path, PathBuf}};

/// The directory a title's persistent storage lives in under `base`.
///
/// wie names that namespace by the program id now; it used the application id before, and the two
/// differ for the archives that carry both. Every title has a data directory of its own here, so a
/// namespace directory that does not exist yet while exactly one other one does is that title's
/// earlier namespace, and it is renamed rather than left behind with the saves in it.
pub fn namespace_dir(base: &Path, id: &str) -> PathBuf {
    let dir = base.join(id);
    if dir.exists() {
        return dir;
    }

    let Ok(entries) = fs::read_dir(base) else {
        return dir;
    };
    let others = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    if let [legacy] = others.as_slice() {
        match fs::rename(legacy, &dir) {
            Ok(()) => tracing::info!("moved storage {} to {}", legacy.display(), dir.display()),
            Err(error) => tracing::warn!("could not move storage {} to {}: {error}", legacy.display(), dir.display()),
        }
    }

    dir
}

#[cfg(test)]
mod tests {
    use super::namespace_dir;

    #[test]
    fn a_lone_earlier_namespace_moves_to_the_new_id() {
        let base = std::env::temp_dir().join(format!("wipi-storage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("AID0001")).unwrap();
        std::fs::write(base.join("AID0001").join("save"), b"x").unwrap();

        let dir = namespace_dir(&base, "PID0001");
        assert_eq!(std::fs::read(dir.join("save")).unwrap(), b"x");
        assert!(!base.join("AID0001").exists());

        // with two namespaces nothing is guessed
        std::fs::create_dir_all(base.join("OTHER")).unwrap();
        assert!(!namespace_dir(&base, "NEW").exists());

        std::fs::remove_dir_all(&base).unwrap();
    }
}
