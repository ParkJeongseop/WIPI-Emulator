// wie_cli/src/database.rs를 base 경로만 주입받도록 옮긴 것.
use std::{fs, path::PathBuf};

use wie_backend::RecordId;

pub struct FsDatabaseRepository {
    base_path: PathBuf,
}

impl FsDatabaseRepository {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn get_path_for_database(&self, name: &str, app_id: &str) -> PathBuf {
        let sanitized_app_id: String = app_id.chars().filter(|c| !matches!(c, '/' | '\\' | '\0')).collect();
        let app_id = if sanitized_app_id.is_empty() || sanitized_app_id == "." || sanitized_app_id == ".." {
            "_"
        } else {
            &sanitized_app_id
        };

        let name: String = name.chars().map(|c| if matches!(c, '\\' | '\0') { '_' } else { c }).collect();
        let mut normalized_name = PathBuf::new();
        for segment in name.trim_start_matches('/').split('/') {
            match segment {
                "" | "." => {}
                ".." => normalized_name.push("_"),
                segment => normalized_name.push(segment),
            }
        }
        if normalized_name.as_os_str().is_empty() {
            normalized_name.push("_");
        }

        self.base_path.join(app_id).join(normalized_name)
    }
}

#[async_trait::async_trait]
impl wie_backend::DatabaseRepository for FsDatabaseRepository {
    async fn open(&self, name: &str, app_id: &str) -> Box<dyn wie_backend::Database> {
        let path = self.get_path_for_database(name, app_id);

        Box::new(Database::new(path).unwrap())
    }

    async fn exists(&self, name: &str, app_id: &str) -> bool {
        self.get_path_for_database(name, app_id).exists()
    }

    async fn usage(&self, app_id: &str) -> u64 {
        fn directory_usage(path: &std::path::Path) -> u64 {
            let Ok(entries) = fs::read_dir(path) else {
                return 0;
            };
            entries
                .filter_map(Result::ok)
                .map(|entry| {
                    let Ok(file_type) = entry.file_type() else {
                        return 0;
                    };
                    if file_type.is_file() {
                        entry.metadata().map(|metadata| metadata.len()).unwrap_or(0)
                    } else if file_type.is_dir() {
                        directory_usage(&entry.path())
                    } else {
                        0
                    }
                })
                .sum()
        }

        directory_usage(&self.base_path.join(app_id))
    }

    async fn delete(&self, name: &str, app_id: &str) -> bool {
        let path = self.get_path_for_database(name, app_id);

        match fs::remove_dir_all(path) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                tracing::warn!("Failed to delete database: {e}");
                false
            }
        }
    }
}

pub struct Database {
    base_path: PathBuf,
}

impl Database {
    pub fn new(base_path: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&base_path)?;

        Ok(Self { base_path })
    }

    fn find_empty_record_id(&self) -> RecordId {
        let mut record_id = 1; // XXX midp requires first record to be 1

        loop {
            let path = self.base_path.join(record_id.to_string());

            if !path.exists() {
                return record_id;
            }

            record_id += 1;
        }
    }

    fn get_path_for_record(&self, id: RecordId) -> PathBuf {
        self.base_path.join(id.to_string())
    }
}

#[async_trait::async_trait]
impl wie_backend::Database for Database {
    async fn next_id(&self) -> RecordId {
        self.find_empty_record_id()
    }

    async fn add(&mut self, data: &[u8]) -> RecordId {
        let id = self.find_empty_record_id();

        let path = self.get_path_for_record(id);
        fs::write(path, data).unwrap();

        id
    }

    async fn get(&self, id: RecordId) -> Option<Vec<u8>> {
        fs::read(self.get_path_for_record(id)).ok()
    }

    async fn set(&mut self, id: RecordId, data: &[u8]) -> bool {
        fs::write(self.get_path_for_record(id), data).is_ok()
    }

    async fn delete(&mut self, id: RecordId) -> bool {
        fs::remove_file(self.get_path_for_record(id)).is_ok()
    }

    async fn get_record_ids(&self) -> Vec<RecordId> {
        fs::read_dir(&self.base_path)
            .unwrap()
            .filter(|x| x.as_ref().unwrap().path().is_file())
            .map(|x| x.unwrap().file_name().to_str().unwrap().parse().unwrap())
            .collect()
    }
}
