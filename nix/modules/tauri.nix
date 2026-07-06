{ config, lib, pkgs, ... }:

let
  inherit (pkgs) lib;

  baseBuildInputs = with pkgs; [
    glib.dev gtk3.dev webkitgtk_4_1.dev libsoup_3.dev
    cairo.dev pango.dev gdk-pixbuf.dev atk.dev
    gst_all_1.gstreamer.dev gst_all_1.gst-plugins-base.dev
  ];

  baseLibInputs = with pkgs; [
    glib gtk3 webkitgtk_4_1 libsoup_3
    cairo pango gdk-pixbuf atk
    libxkbcommon
    stdenv.cc.cc.lib
    gst_all_1.gstreamer
    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-good
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-ugly
    gst_all_1.gst-libav
  ];

  ldLibPath = lib.makeLibraryPath baseLibInputs;

  # Manually construct the pkg-config path from the dev inputs
  pkgConfigPath = lib.concatStringsSep ":" [
    (lib.makeSearchPath "lib/pkgconfig" baseBuildInputs)
    (lib.makeSearchPath "share/pkgconfig" baseBuildInputs)
  ];

  gstPluginPath = with pkgs.gst_all_1; lib.concatStringsSep ":" [
    "${gstreamer}/lib/gstreamer-1.0"
    "${gst-plugins-base}/lib/gstreamer-1.0"
    "${gst-plugins-good}/lib/gstreamer-1.0"
    "${gst-plugins-bad}/lib/gstreamer-1.0"
    "${gst-plugins-ugly}/lib/gstreamer-1.0"
    "${gst-libav}/lib/gstreamer-1.0"
  ];

  mkTauriScript = name: pnpmCmd: pkgs.stdenv.mkDerivation {
    inherit name;
    buildInputs = baseBuildInputs;
    phases = [ "installPhase" ];
    installPhase = ''
      mkdir -p $out/bin
      cat > $out/bin/${name} << 'WRAPPER'
#!/usr/bin/env bash
set -euo pipefail
export PATH="__PKG_CONFIG_BIN__:$PATH"
export PKG_CONFIG_PATH="__PKG_CONFIG_PATH__"
export LD_LIBRARY_PATH="__LD_LIB_PATH__"
export GST_PLUGIN_SYSTEM_PATH="__GST_PLUGIN_PATH__"
export GST_PLUGIN_PATH="__GST_PLUGIN_PATH__"
export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
export WEBKIT_USE_GL=software
exec nix run --impure github:nix-community/nixGL -- __PNPM_CMD__
WRAPPER
      sed -i "s|__PKG_CONFIG_BIN__|${pkgs.pkg-config}/bin|g" $out/bin/${name}
      sed -i "s|__PKG_CONFIG_PATH__|${pkgConfigPath}|g" $out/bin/${name}
      sed -i "s|__LD_LIB_PATH__|${ldLibPath}|g" $out/bin/${name}
      sed -i "s|__GST_PLUGIN_PATH__|${gstPluginPath}|g" $out/bin/${name}
      sed -i "s|__PNPM_CMD__|${pnpmCmd}|g" $out/bin/${name}
      chmod +x $out/bin/${name}
    '';
  };
in
{
  environment.systemPackages = [
    (mkTauriScript "tauri-run"    "pnpm run tauri dev")
    (mkTauriScript "tauri-build" "pnpm tauri build --bundles deb,rpm")
  ];
}
