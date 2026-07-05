import { invoke } from '@tauri-apps/api/core';

export interface VideoPrepareResult {
  url: string;
  action: string;
}

export async function prepareVideo(
  filePath: string,
  playerId: string = 'default',
  force: string | null = null
): Promise<VideoPrepareResult> {
  return invoke<VideoPrepareResult>('prepare_video', { filePath, playerId, force });
}

export async function cancelVideoPrepare(playerId: string = 'default'): Promise<void> {
  return invoke('cancel_video_prepare', { playerId });
}

export async function clearVideoCache(): Promise<void> {
  return invoke('clear_video_cache');
}
