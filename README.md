# Protonic

An app with a GUI for launching Windows executablesc (.exe) files inside almost any Steam games' Proton environments using [protonhax](https://github.com/jcnils/protonhax).

## Features

- Browse and select from your installed Steam games.
- Select one or even two of your own `.exe` files to inject/run next to your game. 
- Now includes per-game config memory that persists between sessions
- **Auto-configure launch options** : automatically adds protonhax to Steam's launch options (preserves existing options)
- **Audio feedback** : New audio cues when launching game and your secondary .exe program
- Simple one-click launch with F1 hotkey activation

## Requirements

- Linux with Steam installed
- [protonhax](https://github.com/jcnils/protonhax) installed and in`PATH` (I may bundle this in an installer)
- Rust toolchain (for building, until packages are availabe)

## Building

```bash
cd ~/
git clone https://github.com/LunaBaloona/protonic.git
cd protonic
cargo build --release
```

The Protonic binary/executable will be at `~/protonic/target/release/protonic`.

## Usage

1. Open Protonic and select your game from the list
2. Click **Browse** to select the `.exe` file(s) you want to run
3. Ensure **Auto-configure launch options** is checked (or manually add `protonhax init %COMMAND%` to your game's Steam Launch Options)
4. Click **Launch** — your game will start via Steam
5. Once in-game, press **F1** to launch your selected executable(s)

## Configuration

Protonic's settings are stored in `~/.config/protonic/default-config.toml`.

