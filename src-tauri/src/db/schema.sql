CREATE TABLE IF NOT EXISTS courses (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,
    vault_path TEXT NOT NULL,
    source_type TEXT CHECK (source_type IN ('github','gitlab','codeberg','pasted')),
    origin_url TEXT,
    content_hash TEXT,
    imported_at TEXT,
    archived_at TEXT
);

CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS course_categories (
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    PRIMARY KEY (course_id, category_id)
);

CREATE TABLE IF NOT EXISTS course_sections (
    id TEXT PRIMARY KEY,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    parent_section_id TEXT REFERENCES course_sections(id) ON DELETE CASCADE,
    canonical_path TEXT NOT NULL,
    vault_path TEXT NOT NULL,
    title TEXT NOT NULL,
    heading_level INTEGER NOT NULL,
    order_index INTEGER NOT NULL,
    UNIQUE (course_id, canonical_path)
);

CREATE INDEX IF NOT EXISTS idx_sections_course_parent ON course_sections(course_id, parent_section_id, order_index);

CREATE TABLE IF NOT EXISTS course_paths (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    vault_path TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS course_path_items (
    course_path_id TEXT NOT NULL REFERENCES course_paths(id) ON DELETE CASCADE,
    course_id TEXT NOT NULL REFERENCES courses(id) ON DELETE CASCADE,
    order_index INTEGER NOT NULL,
    is_optional INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (course_path_id, course_id)
);

CREATE TABLE IF NOT EXISTS section_progress (
    section_id TEXT PRIMARY KEY REFERENCES course_sections(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'not_started' CHECK (status IN ('not_started','in_progress','completed')),
    completed_at TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS section_search USING fts5(title, content, tokenize='porter');
