{
 description = "RP2040 Development";

 inputs = {
   nixpkgs.url = "flake:nixpkgs";
   fenix = {
     url = "github:nix-community/fenix";
     inputs.nixpkgs.follows = "nixpkgs";
   };
 };

 outputs = { self, ... }@inputs:
 let
   system = "x86_64-linux";

   pkgs = inputs.nixpkgs.legacyPackages.${system};

   toolchain = inputs.fenix.packages.${system}.fromToolchainFile {
     file = ./rust-toolchain.toml;
     sha256 = "sha256-CvIrHO77ukowaW3l6NNfWh38nfIRBGsN2jEqQvo+RIs=";
   };

   python = pkgs.python3.withPackages (ps: with ps; [
     matplotlib
     pyserial
   ]);
 in
 {
   devShells.${system}.default = pkgs.mkShell {
     buildInputs = [
       toolchain
       pkgs.probe-rs
       python
     ];
   };
 };
}
