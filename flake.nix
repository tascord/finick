{
  description = "Finick - A Rust/Tauri application with Vue frontend";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    devenv.url = "github:cachix/devenv";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, devenv, ... }@inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "wasm32-unknown-unknown" ];
        };

        # System dependencies for Tauri
        systemDeps = with pkgs; [
          # Tauri dependencies
          pkg-config
          openssl
          webkitgtk_4_1
          gtk3
          cairo
          gdk-pixbuf
          glib
          dbus
          librsvg
          
          # Additional GTK/GDK dependencies
          atk
          pango
          
          # Build tools
          cmake
          
          # For WebKit
          libsoup_3
          
          # Database
          sqlite
        ];

        # Development tools
        devTools = with pkgs; [
          rustToolchain
          nodejs_20
          yarn
          # Tauri CLI will be installed via cargo
          
          # Additional development tools
          git
          jq
          curl
          
          # Optional but useful
          rust-analyzer
          cargo-watch
          cargo-edit
          cargo-audit
        ];

        # Runtime libraries
        runtimeLibs = with pkgs; [
          libxkbcommon
          libGL
          wayland
          xorg.libXcursor
          xorg.libXrandr
          xorg.libXi
          xorg.libX11
        ];

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = systemDeps ++ devTools;

          shellHook = ''
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPath "lib/pkgconfig" [
              pkgs.openssl.dev
              pkgs.webkitgtk_4_1.dev
              pkgs.gtk3.dev
              pkgs.cairo.dev
              pkgs.gdk-pixbuf.dev
              pkgs.glib.dev
              pkgs.dbus.dev
              pkgs.atk.dev
              pkgs.pango.dev
              pkgs.librsvg.dev
              pkgs.libsoup_3.dev
              pkgs.sqlite.dev
            ]}"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (systemDeps ++ runtimeLibs)}"
            export WEBKIT_DISABLE_DMABUF_RENDERER=1
            export NODE_ENV=development

            echo "🦀 Welcome to Finick development environment!"
            echo ""
            echo "Available commands:"
            echo "  cargo build              - Build Rust workspace"
            echo "  cargo tauri dev          - Start Tauri development server"
            echo "  cd scan && yarn install  - Install frontend dependencies"
            echo "  cd scan && yarn dev      - Start Vue development server"
            echo ""
            echo "Rust toolchain: $(rustc --version)"
            echo "Node.js: $(node --version)"
            echo "Yarn: $(yarn --version)"
            echo ""
            
            # Install Tauri CLI if not present
            if ! command -v cargo-tauri &> /dev/null; then
              echo "Installing Tauri CLI..."
              cargo install tauri-cli
            fi
          '';
        };

        # Alternative devenv shell for those who want devenv features
        devShells.devenv = devenv.lib.mkShell {
          inherit pkgs inputs;
          modules = [
            ({ pkgs, config, ... }: {
              # Import devenv module configuration
              imports = [ ./devenv.nix ];
            })
          ];
        };

        # Default package - build the workspace
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "finick";
          version = "0.1.0";
          
          src = ./.;
          
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          
          nativeBuildInputs = systemDeps;
          buildInputs = systemDeps ++ runtimeLibs;
          
          # Skip tests for now as they might need additional setup
          doCheck = false;
          
          meta = with pkgs.lib; {
            description = "Finick - A Rust/Tauri application";
            license = licenses.mit; # Adjust as needed
            maintainers = [ ];
          };
        };

        # Formatter for `nix fmt`
        formatter = pkgs.nixpkgs-fmt;
      });
}