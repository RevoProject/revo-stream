{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # ---------------------------------------------------------------
    # Build / packaging tools
    # ---------------------------------------------------------------
    pkg-config
    xdg-utils

    # ---------------------------------------------------------------
    # GLib / GTK / GObject
    # ---------------------------------------------------------------
    glib
    glib.dev

    gobject-introspection
    gsettings-desktop-schemas

    gtk3
    gtk3.dev

    gdk-pixbuf
    gdk-pixbuf.dev

    cairo
    cairo.dev

    pango
    pango.dev

    atk
    atk.dev

    librsvg

    # ---------------------------------------------------------------
    # WebKit / HTTP
    # ---------------------------------------------------------------
    webkitgtk_4_1
    webkitgtk_4_1.dev

    libsoup_3
    libsoup_3.dev
    curl

    # ---------------------------------------------------------------
    # GLib / libsoup runtime dependencies
    #
    # These are explicitly available for the AppImage dependency
    # collection step.
    # ---------------------------------------------------------------
    dbus

    libpsl
    brotli
    nghttp2
    libffi
    pcre2
    zlib
    libunistring
    libidn2
    libselinux
    util-linux

    # ---------------------------------------------------------------
    # Linux / graphics
    # ---------------------------------------------------------------
    libxkbcommon

    libxcomposite
    libxdamage
    libxfixes
    libxrandr
    libxrender
    libXext
    libXtst

    libGL
    mesa

    # ---------------------------------------------------------------
    # GStreamer
    # ---------------------------------------------------------------
    gst_all_1.gstreamer
    gst_all_1.gstreamer.dev

    gst_all_1.gst-plugins-base
    gst_all_1.gst-plugins-base.dev

    gst_all_1.gst-plugins-good
    gst_all_1.gst-plugins-bad
    gst_all_1.gst-plugins-ugly
    gst_all_1.gst-libav

    # ---------------------------------------------------------------
    # OBS
    # ---------------------------------------------------------------
    obs-studio

    # ---------------------------------------------------------------
    # Rust / bindgen
    # ---------------------------------------------------------------
    llvmPackages.libclang
    simde

    # ---------------------------------------------------------------
    # Desktop automation
    # ---------------------------------------------------------------
    wmctrl
    xdotool
    xwininfo

    # ---------------------------------------------------------------
    # Audio
    # ---------------------------------------------------------------
    pulseaudio
    pipewire

    # ---------------------------------------------------------------
    # Crypto / signing
    # ---------------------------------------------------------------
    libgcrypt
    gpgme
  ];

  shellHook = ''
    # ----------------------------------------------------------------
    # pkg-config
    # ----------------------------------------------------------------

    export PKG_CONFIG_PATH="${pkgs.glib.dev}/lib/pkgconfig:${pkgs.gobject-introspection}/lib/pkgconfig:${pkgs.gtk3.dev}/lib/pkgconfig:${pkgs.gdk-pixbuf.dev}/lib/pkgconfig:${pkgs.webkitgtk_4_1.dev}/lib/pkgconfig:${pkgs.libsoup_3.dev}/lib/pkgconfig:${pkgs.cairo.dev}/lib/pkgconfig:${pkgs.pango.dev}/lib/pkgconfig:${pkgs.atk.dev}/lib/pkgconfig:${pkgs.libxkbcommon}/share/pkgconfig:${pkgs.gst_all_1.gstreamer.dev}/lib/pkgconfig:${pkgs.gst_all_1.gst-plugins-base.dev}/lib/pkgconfig:$PKG_CONFIG_PATH"

    # ----------------------------------------------------------------
    # GStreamer
    # ----------------------------------------------------------------

    export GST_PLUGIN_SYSTEM_PATH="${pkgs.gst_all_1.gstreamer}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-base}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-good}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-bad}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-plugins-ugly}/lib/gstreamer-1.0:${pkgs.gst_all_1.gst-libav}/lib/gstreamer-1.0"

    export GST_PLUGIN_PATH="$GST_PLUGIN_SYSTEM_PATH"
    export GST_REGISTRY_REUSE_PLUGIN_SCANNER="no"

    # ----------------------------------------------------------------
    # Rust / bindgen
    # ----------------------------------------------------------------

    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"

    export BINDGEN_EXTRA_CLANG_ARGS="-idirafter ${pkgs.glibc.dev}/include -idirafter ${pkgs.simde}/include"

    # ----------------------------------------------------------------
    # REVO_ROOT
    # ----------------------------------------------------------------

    export REVO_ROOT="$(pwd)/.local/revo-root"

    # ----------------------------------------------------------------
    # Runtime library search path for development/testing
    # ----------------------------------------------------------------

    export LD_LIBRARY_PATH="${pkgs.obs-studio}/lib:${pkgs.webkitgtk_4_1}/lib:${pkgs.libsoup_3}/lib:${pkgs.gtk3}/lib:${pkgs.glib}/lib:${pkgs.gdk-pixbuf}/lib:${pkgs.cairo}/lib:${pkgs.pango}/lib:${pkgs.atk}/lib:${pkgs.dbus}/lib:${pkgs.libGL}/lib:${pkgs.mesa}/lib:${pkgs.libxcomposite}/lib:${pkgs.libxdamage}/lib:${pkgs.libxfixes}/lib:${pkgs.libxrandr}/lib:${pkgs.libxrender}/lib:${pkgs.libXext}/lib:${pkgs.libXtst}/lib:$LD_LIBRARY_PATH"

    # ----------------------------------------------------------------
    # WebKit
    # ----------------------------------------------------------------

    export WEBKIT_DISABLE_COMPOSITING_MODE=1
    export WEBKIT_DISABLE_DMABUF_RENDERER=1
    export WEBKIT_USE_GL=software

    # ----------------------------------------------------------------
    # GLib / GSettings schemas
    # ----------------------------------------------------------------

    GSETTINGS_SCHEMA_SOURCE="$(find \
      "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas" \
      -type d \
      -path '*/glib-2.0/schemas' \
      2>/dev/null | head -1)"

    if [ -n "$GSETTINGS_SCHEMA_SOURCE" ] && [ -f "$GSETTINGS_SCHEMA_SOURCE/gschemas.compiled" ]; then
      export GSETTINGS_SCHEMA_DIR="$GSETTINGS_SCHEMA_SOURCE"
      echo "Using GSettings schemas: $GSETTINGS_SCHEMA_SOURCE"
    else
      echo "WARNING: No compiled GSettings schemas found"
    fi

    # ----------------------------------------------------------------
    # OBS resources
    # ----------------------------------------------------------------

    OBS_STORE="${pkgs.obs-studio}"

    if [ -d "$OBS_STORE" ] && [ ! -f "$REVO_ROOT/share/obs/libobs/default.effect" ]; then
      mkdir -p "$REVO_ROOT/share/obs/libobs"

      for f in "$OBS_STORE/share/obs/libobs/"*; do
        [ -e "$f" ] && ln -sf "$f" "$REVO_ROOT/share/obs/libobs/"
      done

      mkdir -p "$REVO_ROOT/core/lib"
      ln -sfT "$OBS_STORE/lib/obs-plugins" "$REVO_ROOT/core/lib/obs-plugins"

      mkdir -p "$REVO_ROOT/core/share"
      ln -sfT "$OBS_STORE/share/obs" "$REVO_ROOT/core/share/obs"
    fi
  '';
}