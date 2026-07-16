use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppSettings {
    pub vault_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppStatus {
    pub vault_path: String,
    pub courses_dir_exists: bool,
    pub paths_dir_exists: bool,
    pub categories_file_exists: bool,
    pub vault_git_initialized: bool,
}
