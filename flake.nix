{
  description = "NullVoidOS — development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in {
        devShells.default = pkgs.mkShell {
          name = "nullvoidos-dev";

          packages = with pkgs; [
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
          ];

          shellHook = ''
            echo "NullVoidOS dev shell"
            echo "tools: shellcheck, shfmt, yamllint, yq, cosign, skopeo, just, pre-commit, gh"
          '';
        };
      });
}
