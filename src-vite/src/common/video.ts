import { invoke } from '@tauri-apps/api/core';

export interface VideoPrepareResult {
  url: string;
  action: string;
}

/** Host prepare modes: null = strategy auto; process = force transcode; fallback = escalate remux→transcode. */
export type VideoPrepareMode = 'compatible' | 'process' | 'fallback' | null;

/** Formats that WebView video elements generally cannot play reliably. */
export function isWebViewVideoPlaybackDisabled(filePath: string): boolean {
  const extension = filePath.match(/\.([^./\\]+)$/)?.[1]?.trim().toLowerCase() || '';
  return ['mpg', 'mpeg', 'vob'].includes(extension);
}

export async function prepareVideo(
  filePath: string,
  playerId: string = 'default',
  force: VideoPrepareMode = null,
): Promise<VideoPrepareResult> {
  // Backend understands process/fallback; "compatible" means default strategy.
  const hostForce = force === 'compatible' ? null : force;
  return invoke<VideoPrepareResult>('prepare_video', { filePath, playerId, force: hostForce });
}

export async function cancelVideoPrepare(playerId: string = 'default'): Promise<void> {
  return invoke('cancel_video_prepare', { playerId });
}

export async function clearVideoCache(): Promise<void> {
  return invoke('clear_video_cache');
}
