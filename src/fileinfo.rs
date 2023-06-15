use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::Hasher;
use std::io::{BufReader, Read};
use std::os::windows::prelude::MetadataExt;

use serde::{Deserialize, Serialize};
// use serde_json;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfo {
    #[serde(skip)]
    pub path: std::path::PathBuf,
    pub path_hash: String,
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DirInfo {
    pub path: std::path::PathBuf,
    pub files: Vec<FileInfo>,
    pub subdirs: Vec<DirInfo>,
}

impl DirInfo {
    pub fn set_all_file_paths(&mut self, root: &std::path::PathBuf) {
        let dir_path = root.join(&self.path);
        for file in &mut self.files {
            file.path = dir_path.join(file.name.as_str())
        }
        for subdir in &mut self.subdirs {
            subdir.set_all_file_paths(root);
        }
    }

    pub fn flat_hashes(&self) -> HashMap<&str, &FileInfo> {
        fn flat_dir<'a>(dir: &'a DirInfo, map: &mut HashMap<&'a str, &'a FileInfo>) {
            for file in &dir.files {
                map.insert(file.path_hash.as_str(), file);
            }
            for subdir in &dir.subdirs {
                flat_dir(subdir, map);
            }
        }
        let mut result = HashMap::new();
        flat_dir(self, &mut result);
        result
    }

    pub fn diff_with<'a>(&self, base: &'a DirInfo) -> Vec<&'a FileInfo> {
        let mut result = Vec::new();
        let base_hashes = base.flat_hashes();
        let self_hashes = self.flat_hashes();
        for (key, value) in base_hashes {
            if !self_hashes.contains_key(&key) || self_hashes.get(&key).unwrap().hash != value.hash
            {
                result.push(value)
            }
        }
        result
    }
}

impl FileInfo {
    pub fn new(path: &std::path::PathBuf) -> Result<Self, std::io::Error> {
        let p = std::path::Path::new(path);
        assert!(p.exists() && p.is_file());
        let meta = fs::metadata(p)?;
        let file_size = meta.file_size();
        let last_write_time = meta.last_write_time();
        let file = fs::File::open(&p)?;
        let mut reader = BufReader::new(file);

        let mut hasher: DefaultHasher = DefaultHasher::new();
        let mut buffer = [0; 1024];
        loop {
            let bytes_read = reader.read(&mut buffer).unwrap();
            if bytes_read == 0 {
                break;
            }
            hasher.write(&buffer[0..bytes_read])
        }

        let result = hasher.finish();
        let hash = format!("{:x}", result).to_string();
        Ok(Self {
            path: p.to_path_buf(),
            // path_hash: get_hash(path.as_bytes()),
            path_hash: "".to_string(),
            name: p.file_name().unwrap().to_str().unwrap().to_string(),
            size: file_size,
            modified: last_write_time,
            hash,
        })
    }
}

impl DirInfo {
    pub fn new(dir: &std::path::PathBuf) -> Result<Self, std::io::Error> {
        assert!(dir.is_dir());
        let mut dir_info = DirInfo {
            path: dir.clone(),
            files: Vec::new(),
            subdirs: Vec::new(),
        };

        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                dir_info.subdirs.push(DirInfo::new(&dir.join(path))?);
            } else {
                dir_info.files.push(FileInfo::new(&dir.join(path))?);
            }
        }
        Ok(dir_info)
    }

    pub fn strip_root(&mut self) {
        fn strip_dir_info(dir: &mut DirInfo, root: &std::path::PathBuf) {
            dir.path = dir.path.strip_prefix(root).unwrap().to_path_buf();
            for subdir in &mut dir.subdirs {
                strip_dir_info(subdir, root);
            }
            for file in &mut dir.files {
                file.path = file.path.strip_prefix(root).unwrap().to_path_buf();
                let file_path = file.path.to_str().unwrap().replace("\\", "/");
                file.path_hash = get_hash(file_path.as_bytes());
            }
        }
        let root = self.path.clone();
        strip_dir_info(self, &root);
    }
}

pub fn get_hash(s: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    hasher.write(s);
    let result = hasher.finish();
    format!("{:x}", result).to_string()
}
