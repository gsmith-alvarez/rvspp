{
  description = "RVS - RISC-V Visual Simulator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      pkgsFor = system: import nixpkgs { inherit system; };
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = import ./default.nix { inherit pkgs; };
          legacy = import ./legacy.nix { inherit pkgs; };
        });

      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = import ./shell.nix { inherit pkgs; };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/rvs";
        };
        legacy = {
          type = "app";
          program = "${self.packages.${system}.legacy}/bin/rvs-legacy";
        };
      });
    };
}
