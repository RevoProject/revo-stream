{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    pkg-config
    xdg-utils
    glib.dev
    gtk3.dev
    webkitgtk_4_1.dev
    libsoup_3.dev
    cairo.dev
    pango.dev
    gdk-pixbuf.dev
    atk.dev
    libxkbcommon
    gst_all_1.gstreamer.dev
    gst_all_1.gst-plugins-base.dev
    gst_all_1.gst-plugins-good
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-ugly
    gst_all_1.gst-libav
    obs-studio
    llvmPackages.libclang
    simde
    wmctrl
    xdotool
    xorg.xwininfo
    pulseaudio
    pipewire
  ];

  shellHook = ''
    export PKG_CONFIG_PATH="${pkgs.glib.dev}/lib/pkgconfig:${pkgs.gtk3.dev}/lib/pkgconfig:${pkgs.webkitgtk_4_1.dev}/lib/pkgconfig:${pkgs.libsoup_3.dev}/lib/pkgconfig:${pkgs.cairo.dev}/lib/pkgconfig:${pkgs.pango.dev}/lib/pkgconfig:${pkgs.gdk-pixbuf.dev}/lib/pkgconfig:${pkgs.atk.dev}/lib/pkgconfig:${pkgs.libxkbcommon}/share/pkgconfig:${pkgs.gst_all_1.gstreamer.dev}/lib/pkgconfig:${pkgs.gst_all_1.gst-plugins-base.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"

    # Runtime GStreamer plugin path — allows the app to find gst-plugins-bad (appsink, hls, etc.)
    export GST_PLUGIN_SYSTEM_PATH="${pkgs.gst_all_1.gstreamer}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0"
    export GST_PLUGIN_PATH="$GST_PLUGIN_SYSTEM_PATH"
    export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
    export BINDGEN_EXTRA_CLANG_ARGS="-idirafter ${pkgs.glibc.dev}/include -idirafter ${pkgs.simde}/include"
  '';
}
