import { invoke } from '@tauri-apps/api/core'

export interface PlaybackOptions {
  /** Optional preset name to use for playback */
  preset?: string;
  /** Starting position in seconds */
  startTime?: number;
  /** Additional mpv arguments */
  extraArgs?: string[];
  /** Window title */
  title?: string;
  /** Whether to enable progress reporting */
  reportProgress?: boolean;
  /** Progress reporting interval in milliseconds */
  progressIntervalMs?: number;
  /** Window configuration options */
  window?: WindowOptions;
  /** Connection timeout in milliseconds */
  connectionTimeoutMs?: number;
}

export interface WindowOptions {
  /** Whether to use a borderless window */
  borderless?: boolean;
  /** Window position [x, y] relative to screen */
  position?: [number, number];
  /** Window size [width, height] */
  size?: [number, number];
  /** Whether to make the window always on top */
  alwaysOnTop?: boolean;
  /** Alpha value for window transparency (0.0-1.0) */
  opacity?: number;
  /** Whether to hide window on startup */
  startHidden?: boolean;
}

export interface VideoEvent {
  /** Video event type */
  type: 'started' | 'paused' | 'resumed' | 'ended' | 'closed' | 'error' | 'progress';
  /** Video ID */
  id: string;
  /** Current position in seconds (for progress events) */
  position?: number;
  /** Total duration in seconds (for progress events) */
  duration?: number;
  /** Percentage of playback (for progress events) */
  percent?: number;
  /** Error message (for error events) */
  message?: string;
}

/**
 * Play a video file or URL
 * @param path Path to the video file or URL
 * @param options Optional playback options
 * @returns Promise with the video ID
 */
export async function play(path: string, options?: PlaybackOptions): Promise<string> {
  // Convert any legacy options format to the new format
  const normalizedOptions: PlaybackOptions = { ...options };
  
  // Handle backward compatibility for volume and fullscreen
  if ('volume' in (options || {})) {
    console.warn('The volume option in PlaybackOptions is deprecated. Use control() to set volume after playback starts.');
  }
  
  if ('fullscreen' in (options || {})) {
    console.warn('The fullscreen option in PlaybackOptions is deprecated. Use window.borderless = true and window.size = [screen.width, screen.height] instead.');
  }
  
  // Handle backward compatibility for windowOptions
  if ('windowOptions' in (options || {})) {
    const windowOpts = (options as any).windowOptions;
    normalizedOptions.window = {
      borderless: windowOpts?.decorated === false,
      position: windowOpts?.x !== undefined && windowOpts?.y !== undefined 
        ? [windowOpts.x, windowOpts.y] 
        : undefined,
      size: windowOpts?.width !== undefined && windowOpts?.height !== undefined 
        ? [windowOpts.width, windowOpts.height] 
        : undefined,
      alwaysOnTop: windowOpts?.alwaysOnTop,
    };
  }
  
  const response = await invoke<{ videoId: string }>('plugin:playa|play', {
    request: {
      path,
      options: normalizedOptions || {},
    },
  });
  
  return response.videoId;
}

/**
 * Control video playback
 * @param videoId ID of the video to control
 * @param command Control command: 'pause', 'resume', 'seek', 'volume'
 * @param value Optional value for commands that require one (seek position, volume level)
 * @returns Promise with control response
 */
export async function control(
  videoId: string,
  command: string,
  value?: number,
): Promise<{
  success: boolean;
  position?: number;
  duration?: number;
  state?: string;
}> {
  return invoke('plugin:playa|control', {
    request: {
      videoId,
      command,
      value,
    },
  });
}

/**
 * Get information about a video
 * @param videoId ID of the video to get information for
 * @returns Promise with video information
 */
export async function getInfo(videoId: string): Promise<{
  videoId: string;
  path: string;
  position: number;
  duration: number;
  volume: number;
  isPaused: boolean;
  speed: number;
  isMuted: boolean;
}> {
  return invoke('plugin:playa|get_info', {
    request: {
      videoId,
    },
  });
}

/**
 * Close a video
 * @param videoId ID of the video to close
 * @returns Promise indicating success
 */
export async function close(videoId: string): Promise<{ success: boolean }> {
  return invoke('plugin:playa|close', {
    request: {
      videoId,
    },
  });
}

/**
 * List available presets
 * @returns Promise with list of available presets
 */
export async function listPresets(): Promise<{
  presets: string[];
  recommended?: string;
}> {
  return invoke('plugin:playa|list_presets', {
    request: {},
  });
}
