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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Github,
    Gitlab,
    Codeberg,
    Pasted,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CourseSource {
    #[serde(rename = "type")]
    pub source_type: SourceType,
    pub origin_url: Option<String>,
    pub content_hash: String,
    pub imported_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CourseManifest {
    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub categories: Vec<String>,
    pub source: CourseSource,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WrittenCourse {
    pub title: String,
    pub slug: String,
    pub vault_path: String,
    pub sections: Vec<WrittenSection>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WrittenSection {
    pub title: String,
    pub canonical_path: String,
    pub vault_path: String,
    pub heading_level: u8,
    pub order_index: usize,
    pub children: Vec<WrittenSection>,
}
