# MPV Configuration with uosc

This directory contains the mpv configuration files for the neatflix-mpvrs project, including the uosc theme implementation.

## Directory Structure

```
mpv_config/
├── fonts/                  # Font files for uosc
│   ├── uosc_icons.otf      # Icon font for uosc
│   └── uosc_textures.ttf   # Texture font for uosc
├── platforms/              # Platform-specific configurations
│   ├── macos/              # macOS-specific files (current implementation)
│   ├── windows/            # Windows-specific files (future implementation)
│   └── linux/              # Linux-specific files (future implementation)
├── scripts/                # mpv script files
│   ├── uosc.lua            # Main uosc loader script
│   └── uosc/               # uosc script files
│       ├── bin/            # Binary utilities for uosc
│       ├── elements/       # UI elements
│       ├── intl/           # Internationalization files
│       ├── lib/            # Library files
│       └── main.lua        # Main uosc script
├── script-opts/            # Script configuration files
│   └── uosc.conf           # uosc configuration
├── input.conf              # Keyboard and mouse input configuration
├── mpv.conf                # Main mpv configuration file
└── README.md               # This file
```

## uosc Implementation

The uosc theme is implemented as follows:

1. **Main Configuration**: The `mpv.conf` file is configured to disable the standard OSC and enable uosc.

2. **Input Configuration**: The `input.conf` file defines keyboard and mouse bindings for uosc commands.

3. **uosc Script**: The `scripts/uosc.lua` file loads the main uosc script from the `scripts/uosc/main.lua` file.

4. **uosc Configuration**: The `script-opts/uosc.conf` file contains the configuration options for uosc.

5. **Fonts**: The `fonts/` directory contains the font files required by uosc.

## Scripts

### uosc.lua

This is the main loader script that finds and loads the uosc main script. It:

1. Gets the script directory using `mp.get_script_directory()`
2. Determines the path to the uosc directory and main.lua file
3. Loads the main.lua script using `dofile()`

### Troubleshooting

If you encounter issues with uosc:

1. Check the mpv logs for any error messages
2. Ensure that the uosc.lua script can find the uosc/main.lua file
3. Make sure all required files are present in the uosc directory
4. Verify that the mpv.conf file has the following settings:
   ```
   osc=no
   osd-bar=no
   border=no
   ```

## Usage

The uosc theme provides a modern, minimalist UI for mpv with the following features:

- Proximity-based UI elements that show and hide based on cursor position
- Minimizable timeline that can transform into a small progress bar
- Context menu with nesting support (right-click or menu key)
- Configurable controls bar
- Fast and efficient thumbnails (requires thumbfast integration)
- UIs for selecting subtitle/audio/video tracks, downloading subtitles, etc.
- Searchable menus
- Mouse wheel controls for seeking, volume, and speed

## Customization

To customize the uosc theme, edit the following files:

- `script-opts/uosc.conf`: Configure uosc options
- `input.conf`: Add or modify keyboard and mouse bindings
- `mpv.conf`: Adjust mpv settings

## Credits

The uosc theme is developed by [tomasklaen](https://github.com/tomasklaen/uosc) and is used under its license terms. 