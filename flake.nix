{
  description = "linuf — a playful terminal alphabet-and-sound toy for kids";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = f:
        nixpkgs.lib.genAttrs systems (system:
          f (import nixpkgs {
            inherit system;
            overlays = [ self.overlays.default ];
          }));
    in
    {
      # Lets a NixOS config pull linuf in as `pkgs.linuf`.
      overlays.default = final: prev: {
        linuf = final.rustPlatform.buildRustPackage {
          pname = "linuf";
          version = (final.lib.importTOML ./Cargo.toml).package.version;

          # Only the files the build reads, so README/flake edits don't rebuild.
          src = final.lib.fileset.toSource {
            root = ./.;
            fileset = final.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };

          cargoLock.lockFile = ./Cargo.lock;

          # rodio → cpal → alsa-sys needs the ALSA headers at build time.
          nativeBuildInputs = [ final.pkg-config final.makeWrapper ];
          buildInputs = [ final.alsa-lib ];

          # linuf shells out to `espeak-ng` at runtime — guarantee it's on PATH.
          postInstall = ''
            wrapProgram $out/bin/linuf \
              --prefix PATH : ${final.lib.makeBinPath [ final.espeak-ng ]}
          '';

          meta = {
            description = "Playful terminal alphabet-and-sound toy for kids (Swedish)";
            mainProgram = "linuf";
            platforms = final.lib.platforms.linux;
          };
        };
      };

      packages = forAllSystems (pkgs: {
        default = pkgs.linuf;
        linuf = pkgs.linuf;
      });

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          # Inherits cargo, rustc, pkg-config and alsa-lib from the package.
          inputsFrom = [ pkgs.linuf ];
          packages = with pkgs; [
            rustfmt
            clippy
            rust-analyzer
            espeak-ng
          ];
          env.RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      });
    };
}
