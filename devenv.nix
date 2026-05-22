{ pkgs, lib, config, inputs, ... }: 

{

   packages = [
    pkgs.git
    pkgs.lld
    pkgs.mold
    pkgs.devenv
    pkgs.openssl
    pkgs.xdotool
    pkgs.stdenv.cc
    pkgs.binutils
];

  enterShell = ''
    echo ""
    echo "Rust toolchain: $(rustc --version)"
    echo ""
    fi
  '';


  languages.rust = {
    enable = true;
    channel = "nightly";
  };

  pre-commit.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
    cargo-check.enable = true;
  };
}