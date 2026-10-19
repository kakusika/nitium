{
  lib,
  craneLib,
  stdenv,

  #= Dependencies
  pkg-config,
  fontconfig,
  freetype,
  wayland,
  libxkbcommon,
  libGL,
  mesa,
  vulkan-loader,
  llvmPackages,
  python3,
}:
let
  src = ../..;
  commonArgs = {
    inherit src;
    pname = "nitium";
    version = "0.1.0";

    cargoExtraArgs = "-p nitium";

    nativeBuildInputs = [
      pkg-config
      llvmPackages.libclang
      python3
    ];

    buildInputs = [
      fontconfig
      freetype
      wayland
      libxkbcommon
      libGL
      mesa
      vulkan-loader
      stdenv.cc.cc.lib
    ];
  };
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;
    doCheck = false;

    meta = {
      description = "Web browser powered by Servo and Slint";
      mainProgram = "nitium";
      license = lib.licenses.mit;
    };
  }
)
