import { invoke } from '@tauri-apps/api/core';
import type {
  ActiveVolume,
  CreateCommand,
  MountCommand,
  OperationResponse,
  SystemProbe,
  UnmountCommand
} from './types';

export function probeSystem(): Promise<SystemProbe> {
  return invoke<SystemProbe>('probe_system');
}

export function listActiveVolumes(): Promise<ActiveVolume[]> {
  return invoke<ActiveVolume[]>('list_active_volumes');
}

export function createVolume(request: CreateCommand): Promise<OperationResponse> {
  return invoke<OperationResponse>('create_volume', { request });
}

export function mountVolume(request: MountCommand): Promise<OperationResponse> {
  return invoke<OperationResponse>('mount_volume', { request });
}

export function unmountVolume(request: UnmountCommand): Promise<OperationResponse> {
  return invoke<OperationResponse>('unmount_volume', { request });
}
