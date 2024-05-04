{
 description = "RP2040 Development";

 inputs = {
   nixpkgs.url = "flake:nixpkgs";
   rust-overlay = {
     url = "github:oxalica/rust-overlay";
     inputs.nixpkgs.follows = "nixpkgs";
   };
 };

 outputs = { self, ... }@inputs:
 let
   system = "x86_64-linux";
   target = "thumbv6m-none-eabi";

   pkgs = import inputs.nixpkgs {
     inherit system;
     overlays = [(import inputs.rust-overlay)];
   };

   crossPkgs = import inputs.nixpkgs {
     inherit system;
     rustc.config = target;
   };

   rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
 in
 {
   devShells.${system}.default = pkgs.mkShell {
     buildInputs = [
       rust
       pkgs.openocd-rp2040
       pkgs.probe-rs
       pkgs.elf2uf2-rs
       pkgs.minicom
     ];
   };
 };
}
