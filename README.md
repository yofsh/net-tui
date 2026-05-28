# net-tui

Terminal UI managers for WiFi (`wifi-tui`) and Bluetooth (`bt-tui`) on Linux.

Built with [ratatui](https://ratatui.rs). Shells out to `nmcli` + `iw` for WiFi and `bluetoothctl` for Bluetooth.

## Install (Nix flake)

```sh
nix run github:yofsh/net-tui#wifi-tui
nix run github:yofsh/net-tui#bt-tui
```

Or add as an input:

```nix
{
  inputs.net-tui.url = "github:yofsh/net-tui";

  # then in your packages:
  environment.systemPackages = [
    inputs.net-tui.packages.${pkgs.system}.wifi-tui
    inputs.net-tui.packages.${pkgs.system}.bt-tui
  ];
}
```

The flake wraps each binary with the required runtime tools on PATH (`networkmanager`, `iw`, `bluez`), so the binaries work out of the box on any NixOS host.

## Build from source

```sh
cargo build --release --workspace
./target/release/wifi-tui
./target/release/bt-tui
```

Runtime requirements: `nmcli` and `iw` for `wifi-tui`, `bluetoothctl` for `bt-tui`.

## Layout

```
crates/
├── core/       # net-tui-core: shared scaffolding (theme, overlay, hotbar, status, runtime, filter)
├── wifi-tui/   # nmcli/iw frontend
└── bt-tui/     # bluetoothctl frontend
```

## Keys (common)

`↑↓/jk` navigate · `g/G` first/last · `/` filter · `h` help · `q` quit · click on the hotbar at the bottom to invoke actions with the mouse.
