{
  pkgs ? import <nixpkgs> { },
}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    openssl
    cargo-llvm-cov
    cargo-fuzz
  ];
}
