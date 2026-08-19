export type ActivationMode = 'hold' | 'toggle';

export interface AppSettings {
  activationMode: ActivationMode;
  modelId: string;
}

export interface Transcript {
  id: string;
  createdAtMs: number;
  durationMs: number;
  text: string;
  appName?: string;
  words: number;
}

export interface UserData {
  settings: AppSettings;
  history: Transcript[];
}

export interface ModelStatus {
  id: string;
  label: string;
  sizeBytes: number;
  downloaded: boolean;
}

export interface RuntimeStatus {
  phase:
    | 'idle'
    | 'starting'
    | 'recording'
    | 'transcribing'
    | 'cleaning'
    | 'inserting'
    | 'complete'
    | 'error';
  mode: 'pushToTalk' | 'handsFree' | 'manual';
  level: number;
  elapsedMs: number;
  message?: string;
  preview?: string;
}

export interface Bootstrap {
  data: UserData;
  models: ModelStatus[];
  runtime: RuntimeStatus;
  shortcutReady: boolean;
  autoInsertReady: boolean;
  platform: string;
}
