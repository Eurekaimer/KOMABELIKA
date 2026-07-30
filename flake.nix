{
  description = "Komari Call terminal companion";

  inputs.nixpkgs.url = "https://channels.nixos.org/nixos-25.11/nixexprs.tar.xz";

  outputs =
    { self, nixpkgs, ... }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          source = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              let
                name = builtins.baseNameOf path;
              in
              pkgs.lib.cleanSourceFilter path type
              && name != "target"
              && name != "result"
              && name != ".local-data";
          };
          package = pkgs.rustPlatform.buildRustPackage {
            pname = "komari-call";
            version = "0.1.0";
            src = source;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.pkg-config ];

            meta = {
              description = "A terminal companion for quiet conversations";
              homepage = "https://github.com/Eurekaimer/KOMABELIKA";
              license = pkgs.lib.licenses.gpl3Plus;
              mainProgram = "komari-call";
              platforms = pkgs.lib.platforms.linux;
            };
          };
        in
        {
          komari-call = package;
          default = package;
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/komari-call";
          meta.description = "Chat with Komari Call in the terminal";
        };
      });

      nixosModules.default =
        { pkgs, ... }:
        {
          environment.systemPackages = [
            self.packages.${pkgs.stdenv.hostPlatform.system}.default
          ];
        };

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              clippy
              pkg-config
              rust-analyzer
              rustc
              rustfmt
            ];
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          };
        }
      );
    };
}
