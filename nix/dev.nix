{
  lib,
  pkgs,
  mkShell,
  fenix,
  tmtbook,
  tomet,
  tomet-lsp,
  twrit,
  ...
}:
let
  rustToolchain = fenix.combine [
    (fenix.stable.withComponents [
      "cargo"
      "clippy"
      "rustc"
      "rust-src"
      "rustfmt"
      "rust-analyzer"
    ])
  ];
in
mkShell {
  hardeningDisable = [ "fortify" ];
  buildInputs = with pkgs; [
    #= Develop
    tomet
    tomet-lsp
    tmtbook
    twrit
    deno
    #== Slint
    slint-lsp
    slint-viewer
    #== Build
    pkg-config
    cmake
    llvmPackages.libclang
    python3
    uv
    #== Rust
    rustToolchain
    cargo-edit
    cargo-outdated
    cargo-nextest
    wasm-bindgen-cli

    #= Runtime
    wayland
    libxkbcommon
    fontconfig
    freetype
    stdenv.cc.cc.lib # Skia's prebuilt binary links against libstdc++
    #== Graphics
    libGL
    mesa
    vulkan-loader
    vulkan-validation-layers
    vulkan-tools
  ];

  LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
  BINDGEN_EXTRA_CLANG_ARGS =
    "-isystem ${pkgs.stdenv.cc.cc}/include/c++/${pkgs.stdenv.cc.cc.version} "
    + "-isystem ${pkgs.stdenv.cc.cc}/include/c++/${pkgs.stdenv.cc.cc.version}/${pkgs.stdenv.hostPlatform.config} "
    + "-isystem ${pkgs.stdenv.cc.libc.dev}/include";

  PKG_CONFIG_PATH = lib.makeSearchPathOutput "dev" "lib/pkgconfig" [
    pkgs.fontconfig
    pkgs.freetype
    pkgs.wayland
    pkgs.libxkbcommon
  ];

  LD_LIBRARY_PATH =
    lib.makeLibraryPath [
      pkgs.wayland
      pkgs.libxkbcommon
      pkgs.fontconfig
      pkgs.freetype
      pkgs.mesa
      pkgs.libGL
      pkgs.vulkan-loader
      pkgs.stdenv.cc.cc.lib
    ]
    + ":/run/opengl-driver/lib";

  shellHook = ''
    echo "🧪 Rust Nitium"
  '';
}
