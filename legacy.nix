{ pkgs ? import <nixpkgs> {} }:

pkgs.writeShellScriptBin "rvs-legacy" ''
  exec ${pkgs.tk}/bin/wish "${./src/RVS046.tcl}" "$@"
''
