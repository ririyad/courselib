import { invoke } from '@tauri-apps/api/core';

export type AppStatus = {
  vault_path: string;
  courses_dir_exists: boolean;
  paths_dir_exists: boolean;
  categories_file_exists: boolean;
  vault_git_initialized: boolean;
};

export function getAppStatus() {
  return invoke<AppStatus>('get_app_status');
}

export function setVaultPath(path: string) {
  return invoke<AppStatus>('set_vault_path', { path });
}
