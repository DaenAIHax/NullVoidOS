{ stdenvNoCC, fetchurl, lib }:

let
  version = "0.1.4";

  sources = {
    x86_64-linux = {
      asset = "zero-linux-musl-x64";
      sha256 = "734917a037c54197261ba178f28f1e6020674567cbf16392676e3d1cfb3434d2";
    };
    aarch64-linux = {
      asset = "zero-linux-musl-arm64";
      sha256 = "7f163ba80747a328bbc7722085ebeead100257eaf39373d968f7d627599648c4";
    };
    x86_64-darwin = {
      asset = "zero-darwin-x64";
      sha256 = "6e588e3c2381dae1fea0ef1498ecb139fdbc5023d2939a793d24179bee8e6a3e";
    };
    aarch64-darwin = {
      asset = "zero-darwin-arm64";
      sha256 = "ed2f8d90305704a8013588b9a0325438ba862041944dcd0771a4033953e54346";
    };
  };

  system = stdenvNoCC.hostPlatform.system;
  src = sources.${system}
    or (throw "zerolang: unsupported system ${system}");
in
stdenvNoCC.mkDerivation {
  pname = "zerolang";
  inherit version;

  src = fetchurl {
    url = "https://github.com/vercel-labs/zero/releases/download/v${version}/${src.asset}";
    inherit (src) sha256;
  };

  dontUnpack = true;

  installPhase = ''
    runHook preInstall
    install -Dm755 $src $out/bin/zero
    runHook postInstall
  '';

  meta = with lib; {
    description = "Experimental systems programming language designed for AI agents";
    homepage = "https://zerolang.ai";
    license = licenses.asl20;
    platforms = builtins.attrNames sources;
    mainProgram = "zero";
  };
}
