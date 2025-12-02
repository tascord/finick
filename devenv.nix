{ pkgs, lib, config, inputs, ... }: 

let
      # Additional development tools
    git
    jq
    curl
    cargo-watch
    cargo-edit
    cargo-audit
    
    # Database
    sqlite
  ];st overlay from inputs
  rust-overlay = inputs.rust-overlay.overlays.default;
  pkgsWithRust = import inputs.nixpkgs {
    inherit (pkgs) system;
    overlays = [ rust-overlay ];
  };
  
  # Rust toolchain
  rustToolchain = pkgsWithRust.rust-bin.stable.latest.default.override {
    extensions = [ "rust-src" "rust-analyzer" ];
    targets = [ "wasm32-unknown-unknown" ];
  };
in

{
  # This is a devenv.nix configuration file for the project
  # Use with: devenv shell

  packages = with pkgs; [
    # Rust toolchain
    rustToolchain
    
    # Node.js ecosystem
    nodejs_20
    yarn
    
    # System dependencies for Tauri
    pkg-config
    openssl
    webkitgtk_4_1
    gtk3
    cairo
    gdk-pixbuf
    glib
    dbus
    librsvg
    atk
    pango
    cmake
    libsoup_3
    
    # Development tools
    git
    jq
    curl
    cargo-watch
    cargo-edit
    cargo-audit
  ];

  env = {
    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
    PKG_CONFIG_PATH = lib.makeSearchPath "lib/pkgconfig" [
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
    ];
    LD_LIBRARY_PATH = lib.makeLibraryPath [
      pkgs.openssl
      pkgs.webkitgtk_4_1
      pkgs.gtk3
      pkgs.cairo
      pkgs.gdk-pixbuf
      pkgs.glib
      pkgs.dbus
      pkgs.librsvg
      pkgs.atk
      pkgs.pango
      pkgs.libsoup_3
      pkgs.libxkbcommon
      pkgs.libGL
      pkgs.wayland
      pkgs.xorg.libXcursor
      pkgs.xorg.libXrandr
      pkgs.xorg.libXi
      pkgs.xorg.libX11
    ];
    WEBKIT_DISABLE_DMABUF_RENDERER = "1";
    NODE_ENV = "development";
  };

  enterShell = ''
    echo "🦀 Welcome to Finick development environment (devenv)!"
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

  scripts = {
    dev-frontend.exec = ''
      cd scan
      yarn install
      yarn dev
    '';
    
    dev-tauri.exec = ''
      cd scan
      yarn install
      cargo tauri dev
    '';
    
    build-all.exec = ''
      echo "Building Rust workspace..."
      cargo build --workspace
      
      echo "Building frontend..."
      cd scan
      yarn install
      yarn build
      
      echo "Building Tauri app..."
      cargo tauri build
    '';
    
    clean-all.exec = ''
      echo "Cleaning Rust build artifacts..."
      cargo clean
      
      echo "Cleaning frontend dependencies..."
      cd scan
      rm -rf node_modules
      rm -rf dist
    '';
  };

  languages = {
    rust.enable = true;
    javascript.enable = true;
    typescript.enable = true;
  };

  pre-commit.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
    cargo-check.enable = true;
  };
}