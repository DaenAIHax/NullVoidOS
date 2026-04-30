{
  description = "NullVoidOS — security-focused immutable NixOS distribution";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, flake-utils, ... }:
    {
      nixosModules.default = ./modules;
    }
    //
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        pkgsUnstable = nixpkgs-unstable.legacyPackages.${system};
      in {
        packages = import ./pkgs { inherit pkgs; };

        devShells.default = pkgsUnstable.mkShell {
          name = "nullvoidos-dev";

          packages = with pkgsUnstable; [
            shellcheck
            shfmt
            bashInteractive
            yamllint
            yq-go
            cosign
            skopeo
            just
            pre-commit
            git
            gh
            nixpkgs-fmt
            nil
            nix-tree
          ];

          shellHook = ''
            echo "NullVoidOS dev shell"
          '';
        };
      });
}
