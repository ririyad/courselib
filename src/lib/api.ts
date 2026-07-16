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

export function getAppStatus() {
  return invoke<AppStatus>('get_app_status');
}

export function setVaultPath(path: string) {
  return invoke<AppStatus>('set_vault_path', { path });
}

export function importCourse(source: ImportCourseSource) {
  return invoke<WrittenCourse>('import_course', { source });
}
