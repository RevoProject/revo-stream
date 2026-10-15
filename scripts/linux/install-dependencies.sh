#!/usr/bin/env bash
set -euo pipefail

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential cmake extra-cmake-modules ninja-build pkg-config clang libclang-dev \
  curl wget file patchelf xdg-utils rpm makeself \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev libssl-dev librsvg2-dev \
  libsimde-dev uthash-dev libjansson-dev nlohmann-json3-dev \
  libcurl4-openssl-dev libmbedtls-dev uuid-dev \
  libavcodec-dev libavdevice-dev libavfilter-dev libavformat-dev \
  libavutil-dev libswresample-dev libswscale-dev libx264-dev \
  libx11-dev libx11-xcb-dev libxcb-randr0-dev libxcb-shm0-dev \
  libxcb-xinerama0-dev libxcb-composite0-dev libxcb-xfixes0-dev \
  libxinerama-dev libxcomposite-dev libxss-dev libxkbcommon-dev \
  libgl1-mesa-dev libegl1-mesa-dev libwayland-dev \
  libpipewire-0.3-dev libpulse-dev libasound2-dev \
  libjack-jackd2-dev libsndio-dev libspeexdsp-dev \
  libfontconfig-dev libfreetype6-dev libudev-dev libv4l-dev \
  libva-dev libpci-dev libdrm-dev librist-dev libsrt-openssl-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-tools gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
  gstreamer1.0-plugins-ugly gstreamer1.0-libav
