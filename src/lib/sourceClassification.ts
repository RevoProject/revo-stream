export const AUDIO_CAPTURE_TYPE_PREFIXES = ["wasapi_", "pipewire_", "pulse_", "alsa_"] as const;

export function isAudioCaptureSourceType(type: string | undefined): boolean {
  const value = (type ?? "").toLowerCase();
  return (
    AUDIO_CAPTURE_TYPE_PREFIXES.some((prefix) => value.startsWith(prefix)) ||
    value.includes("audio_input") ||
    value.includes("audio_output") ||
    value.includes("audio_capture") ||
    value.includes("audio_line")
  );
}

export function isVideoLikeSourceType(type: string | undefined): boolean {
  const value = (type ?? "").toLowerCase();
  return (
    value.includes("video") ||
    value.includes("v4l2") ||
    value.includes("camera") ||
    value.includes("webcam") ||
    value.includes("dshow") ||
    value.includes("window") ||
    value.includes("xcomposite") ||
    value.includes("display") ||
    value.includes("screen") ||
    value.includes("game_capture")
  );
}
