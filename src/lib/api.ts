import { invoke } from '@tauri-apps/api/core';

export type AppStatus = {
  vault_path: string;
  courses_dir_exists: boolean;
  paths_dir_exists: boolean;
  categories_file_exists: boolean;
  vault_git_initialized: boolean;
};

export type WrittenSection = {
  title: string;
  canonical_path: string;
  vault_path: string;
  heading_level: number;
  order_index: number;
  children: WrittenSection[];
};

export type WrittenCourse = {
  title: string;
  slug: string;
  vault_path: string;
  sections: WrittenSection[];
};

export type ImportCourseSource =
  | { Link: { url: string } }
  | { Pasted: { content: string; title_hint?: string | null } };

export type ReindexSummary = {
  courses: number;
  sections: number;
  categories: number;
  paths: number;
};

export type CourseListItem = {
  id: string;
  slug: string;
  title: string;
  description: string | null;
  categories: string[];
  section_count: number;
};

export type SectionNode = {
  id: string;
  canonical_path: string;
  title: string;
  heading_level: number;
  order_index: number;
  children: SectionNode[];
};

export type CourseDetail = {
  id: string;
  slug: string;
  title: string;
  description: string | null;
  categories: string[];
  sections: SectionNode[];
};

export type SectionContent = {
  id: string;
  course_id: string;
  canonical_path: string;
  title: string;
  raw_markdown: string;
  html: string;
};

export function getAppStatus() {
  return invoke<AppStatus>('get_app_status');
}

export function setVaultPath(path: string) {
  return invoke<AppStatus>('set_vault_path', { path });
}

export function importCourse(source: ImportCourseSource) {
  return invoke<WrittenCourse>('import_course', { source });
}

export function listCourses(filter?: { category?: string; include_archived?: boolean }) {
  return invoke<CourseListItem[]>('list_courses', { filter });
}

export function getCourse(courseId: string) {
  return invoke<CourseDetail>('get_course', { courseId });
}

export function getSection(sectionId: string) {
  return invoke<SectionContent>('get_section', { sectionId });
}

export function reindexVault() {
  return invoke<ReindexSummary>('reindex_vault');
}
