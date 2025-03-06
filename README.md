# Tauri Plugin Playa

A Tauri v2 plugin for video playback using mpv. This plugin provides a simple and powerful API for playing videos in Tauri applications by leveraging the mpv media player.

## Features

- Play local video files or streaming URLs with mpv
- Control playback (pause, resume, seek, volume)
- Monitor playback progress and events
- Support for presets optimized for different scenarios (streaming, quality, performance)
- Event-driven API for reacting to playback changes
- Cross-platform support (Windows, macOS, Linux)

## Requirements

- mpv must be installed on the user's system
- Compatible with Tauri v2.0 or newer

## Installation

### Step 1: Add Dependencies

Add the plugin to your Tauri project by adding these dependencies to your `Cargo.toml`:

```toml
[dependencies]
# Use the Git repository directly
tauri-plugin-playa = { git = "https://github.com/0xmrwn/tauri-plugin-playa", tag = "v1" }
```

For the frontend, install the JavaScript/TypeScript API package directly from GitHub:

```bash
# Using npm
npm add https://github.com/0xmrwn/tauri-plugin-playa#v1
# or using yarn
yarn add https://github.com/0xmrwn/tauri-plugin-playa#v1
# or using pnpm
pnpm add https://github.com/0xmrwn/tauri-plugin-playa#v1
```

### Step 2: Register the Plugin

Register the plugin in your Tauri application's `main.rs`:

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_playa::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Step 3: Configure Plugin in Tauri Config

Add the plugin configuration to your `tauri.conf.json` file:

```json
{
  "build": { ... },
  "tauri": { ... },
  "plugins": {
    "playa": {
      "connectionTimeoutMs": 30000
    }
  }
}
```

### Step 4: Set Up Permissions

Tauri v2 uses a permission system to control access to plugin capabilities. Add the necessary permissions to your `tauri.conf.json`:

```json
{
  "tauri": {
    "security": {
      "capabilities": [
        {
          "identifier": "video",
          "description": "Allow video playback",
          "permission": ["allow-play-video", "allow-control-video"]
        }
      ]
    }
  },
  "plugins": { ... }
}
```

## Usage

### JavaScript/TypeScript API

Import the plugin functions in your JavaScript/TypeScript code:

```typescript
// Import from the GitHub package
import { play, control, getInfo, close, listPresets } from 'tauri-plugin-playa-api';
```

#### Playing a Video

```typescript
// Play a video file or URL
const videoId = await play('/path/to/video.mp4', {
  preset: 'streaming',      // Optional preset name
  startTime: 120,           // Start at 2 minutes (optional)
  title: 'My Video',        // Window title (optional)
  reportProgress: true,     // Enable progress reporting (optional)
  progressIntervalMs: 1000, // Progress update interval (optional)
  window: {
    borderless: true,       // Use borderless window (optional)
    position: [100, 100],   // Window position [x, y] (optional)
    size: [800, 600],       // Window size [width, height] (optional)
    alwaysOnTop: false,     // Make window always on top (optional)
    opacity: 1.0,           // Window opacity (optional)
    startHidden: false      // Hide window on startup (optional)
  },
  connectionTimeoutMs: 5000 // Connection timeout (optional)
});
```

#### Controlling Playback

```typescript
// Pause video
await control(videoId, 'pause');

// Resume video
await control(videoId, 'resume');

// Seek to 5 minutes
await control(videoId, 'seek', 300);

// Set volume to 50%
await control(videoId, 'volume', 50);

// Toggle fullscreen
await control(videoId, 'fullscreen');

// Set playback speed
await control(videoId, 'speed', 1.5);

// Mute/unmute audio
await control(videoId, 'mute');
```

#### Getting Playback Information

```typescript
const info = await getInfo(videoId);
console.log(`Position: ${info.position}/${info.duration} seconds`);
console.log(`Volume: ${info.volume}%`);
console.log(`Paused: ${info.isPaused}`);
console.log(`Speed: ${info.speed}x`);
console.log(`Muted: ${info.isMuted}`);
```

#### Closing a Video

```typescript
await close(videoId);
```

#### Listing Available Presets

```typescript
const { presets, recommended } = await listPresets();
console.log(`Recommended preset: ${recommended}`);
console.log('Available presets:', presets);
```

### Events

The plugin emits events that you can listen to using Tauri's event system:

```typescript
import { listen } from '@tauri-apps/api/event';

// Listen for playback events
listen('video:started', (event) => {
  console.log(`Video started: ${event.payload.id}`);
});

listen('video:paused', (event) => {
  console.log(`Video paused: ${event.payload.id}`);
});

listen('video:resumed', (event) => {
  console.log(`Video resumed: ${event.payload.id}`);
});

listen('video:ended', (event) => {
  console.log(`Video ended: ${event.payload.id}`);
});

listen('video:closed', (event) => {
  console.log(`Video closed: ${event.payload.id}`);
});

listen('video:error', (event) => {
  console.error(`Video error: ${event.payload.message}`);
});

// Listen for progress updates
listen('video:progress', (event) => {
  const { id, position, duration, percent } = event.payload;
  console.log(`Progress: ${position}/${duration} (${percent * 100}%)`);
});
```

## Presets

The plugin comes with several presets for different playback scenarios:

- `streaming`: Optimized for streaming videos with lower latency
- `quality`: Prioritizes video quality (higher resolution, better scaling)
- `performance`: Optimized for better performance on lower-end devices
- `mobile`: Optimized for mobile devices with touch controls
- `default`: Balanced settings for most use cases

## Advanced: Custom Permissions

For finer control over plugin capabilities, you can define custom permissions in your plugin's `permissions` directory:

```toml
# permissions/video.toml
"$schema" = "schemas/schema.json"

[[permission]]
identifier = "allow-play-video"
description = "Allows playing video files"
commands.allow = ["play"]

[[permission]]
identifier = "allow-control-video"
description = "Allows controlling video playback"
commands.allow = ["control", "get_info", "close"]

[[permission]]
identifier = "allow-list-presets"
description = "Allows listing available presets"
commands.allow = ["list_presets"]

[[set]]
identifier = "allow-video-full"
description = "Allows full video playback capabilities"
permissions = ["allow-play-video", "allow-control-video", "allow-list-presets"]
```

Then reference these permissions in your app's `tauri.conf.json`:

```json
{
  "tauri": {
    "security": {
      "capabilities": [
        {
          "identifier": "video-player",
          "description": "Video player capabilities",
          "permission": "allow-video-full"
        }
      ]
    }
  }
}
```

## License

MIT
