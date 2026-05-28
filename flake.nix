{
  description = "Terminal UI managers for WiFi and Bluetooth";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      cargoHash = "sha256-P4vRMyapwtmPLSD+ZtKs39YV3KjuRlxmHBTMDRQHcfo=";
      buildBin = { bin, runtimeDeps }:
        pkgs.rustPlatform.buildRustPackage {
          pname = bin;
          version = "0.1.0";
          src = ./.;
          inherit cargoHash;
          cargoBuildFlags = [ "-p" bin ];
          # Run only the tests for this binary's crate (workspace tests would try to build deps we don't need)
          cargoTestFlags = [ "-p" bin ];
          nativeBuildInputs = [ pkgs.makeBinaryWrapper ];
          postFixup = ''
            wrapProgram $out/bin/${bin} \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
          '';
          meta = {
            mainProgram = bin;
            description =
              if bin == "wifi-tui" then "WiFi network manager TUI"
              else "Bluetooth device manager TUI";
            homepage = "https://github.com/yofsh/net-tui";
            license = pkgs.lib.licenses.mit;
            platforms = pkgs.lib.platforms.linux;
          };
        };
    in
    {
      packages.${system} = {
        wifi-tui = buildBin {
          bin = "wifi-tui";
          runtimeDeps = with pkgs; [ networkmanager iw ];
        };
        bt-tui = buildBin {
          bin = "bt-tui";
          runtimeDeps = with pkgs; [ bluez ];
        };
        default = self.packages.${system}.wifi-tui;
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [ rustc cargo rustfmt clippy rust-analyzer pkg-config ];
      };
    };
}
