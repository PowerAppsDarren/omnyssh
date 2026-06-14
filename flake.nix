{
  description = "OmnySSH — TUI SSH dashboard & server manager";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;

        workspaceToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        crateToml = builtins.fromTOML (builtins.readFile ./crates/omnyssh/Cargo.toml);

        workspacePackage = workspaceToml.workspace.package;
        cratePackage = crateToml.package;
        pname = cratePackage.name;
        inherit (workspacePackage) version;
        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit pname version src;

          strictDeps = true;
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        omnyssh = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;

            cargoBuildExtraArgs = "--package ${pname}";
            doCheck = false;

            nativeBuildInputs = commonArgs.nativeBuildInputs ++ [ pkgs.installShellFiles ];

            postInstall = ''
              installManPage doc/omny.1
            '';

            meta = with pkgs.lib; {
              inherit (cratePackage) description;
              homepage = workspacePackage.repository;
              license = licenses.asl20;
              mainProgram = "omny";
            };
          }
        );
      in
      {
        checks = {
          inherit omnyssh;
          omnyssh-fmt = craneLib.cargoFmt { inherit pname version src; };
          omnyssh-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--workspace --all-targets -- --deny warnings";
            }
          );
          omnyssh-test = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoExtraArgs = "--locked --workspace";
            }
          );
        };

        packages = {
          default = omnyssh;
          inherit omnyssh;
        };

        apps.default = {
          type = "app";
          program = "${omnyssh}/bin/omny";
          inherit (omnyssh) meta;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = [
            pkgs.rust-analyzer
          ];

          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
        };
      }
    );
}
