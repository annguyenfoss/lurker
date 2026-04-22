export type VolumeType = 'auto' | 'luks' | 'veracrypt';
export type CreateCipher = 'aes' | 'serpent' | 'twofish';
export type SourceKind = 'file' | 'block';
export type OutputLevel =
  | 'raw_stdout'
  | 'raw_stderr'
  | 'message'
  | 'detail'
  | 'success'
  | 'warning'
  | 'error'
  | 'progress';

export interface OutputEntry {
  level: OutputLevel;
  message: string;
}

export interface ToolStatus {
  name: string;
  path: string | null;
  required: boolean;
}

export interface SystemProbe {
  is_root: boolean;
  tools: ToolStatus[];
}

export interface ActiveVolume {
  mapper_name: string;
  mapper_path: string;
  mountpoint: string | null;
  filesystem_type: string | null;
}

export interface OperationResponse {
  ok: boolean;
  logs: OutputEntry[];
  error: string | null;
}

export interface CreateCommand {
  target: string;
  size_gb: string | null;
  force: boolean;
  source_kind: SourceKind;
  volume_type: Exclude<VolumeType, 'auto'>;
  cipher: CreateCipher;
  passphrase: string | null;
}

export interface MountCommand {
  source: string;
  mountpoint: string;
  tag: string | null;
  volume_type: VolumeType;
  passphrase: string | null;
}

export interface UnmountCommand {
  target: string;
  tag: string | null;
  volume_type: VolumeType;
}
