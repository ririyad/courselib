export type ToastType = 'success' | 'error' | 'info';

export interface Toast {
  id: number;
  message: string;
  type: ToastType;
}

let items = $state<Toast[]>([]);
let nextId = 0;

export function getToasts(): Toast[] {
  return items;
}

export function dismissToast(id: number): void {
  const index = items.findIndex((toast) => toast.id === id);
  if (index !== -1) items.splice(index, 1);
}

export function showToast(message: string, type: ToastType = 'info', duration = 3200): number {
  const id = ++nextId;
  items.push({ id, message, type });
  if (duration > 0) {
    setTimeout(() => dismissToast(id), duration);
  }
  return id;
}
