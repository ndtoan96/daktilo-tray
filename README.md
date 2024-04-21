# Introduction
[daktilo](https://github.com/orhun/daktilo) is a small command-line program that plays typewriter sounds every time you press a key.

**daktilo-tray** brings **daktilo** to the tray. It was tested on Windows, but it should also work on Linux and MacOs.

![Screenshot 2024-04-21 131817](https://github.com/ndtoan96/daktilo-tray/assets/33489972/80de0286-30b2-4146-ad9c-c5ce598376e4)

# Installation
Pre-built binaries are provided at [release](https://github.com/ndtoan96/daktilo-tray/releases).

## Cargo
```bash
cargo install daktilo-tray
```

# Roadmap
- [X] Change preset in realtime
- [X] Change output device in realtime
- [X] Enable/disable app
- [X] Caching app state
- [ ] Configure custom presets
- [ ] Auto detect new audio devices
- [ ] Support other installation methods (nix, winget,...)
