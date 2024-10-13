{ pkgs ? import <nixpkgs> { }, stdenv ? pkgs.stdenv }:
stdenv.mkDerivation rec {
  pname = "appimagetool";
  version = "13";
  src = pkgs.fetchurl {
    url = "https://github.com/AppImage/AppImageKit/releases/download/13/appimagetool-x86_64.AppImage";
    sha256 = "df3baf5ca5facbecfc2f3fa6713c29ab9cefa8fd8c1eac5d283b79cab33e4acb";
  };
  nativeBuildInputs = [
    pkgs.autoPatchelfHook
  ];
  buildInputs = [
    pkgs.fuse
    pkgs.zlib
    stdenv.cc.cc.lib
  ];
  unpackPhase = ''
    cp $src ./appimagetool.AppImage
    chmod 755 ./appimagetool.AppImage
    patchelf --set-interpreter ${stdenv.cc.bintools.dynamicLinker} ./appimagetool.AppImage
    patchelf --add-rpath ${pkgs.fuse}/lib ./appimagetool.AppImage
    patchelf --add-rpath ${pkgs.zlib}/lib ./appimagetool.AppImage
  '';
  installPhase = ''
    runHook preInstall
    ./appimagetool.AppImage --appimage-extract
    install -m755 -d $out
    cp -r ./squashfs-root/usr/bin $out/bin
    cp -r ./squashfs-root/usr/lib $out/lib
    runHook postInstall
  '';
}
