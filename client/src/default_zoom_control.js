import { CAMERA } from "./config.js";

const STORAGE_KEY = "rts.camera.defaultZoom";
const DEFAULT_ZOOM = 1;

export function readDefaultZoom() {
  try {
    const stored = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (stored === null || stored === undefined) return DEFAULT_ZOOM;
    const zoom = Number(stored);
    return Number.isFinite(zoom) && zoom >= CAMERA.minZoom && zoom <= CAMERA.maxZoom
      ? zoom : DEFAULT_ZOOM;
  } catch {
    return DEFAULT_ZOOM;
  }
}

export function renderDefaultZoomControl(root) {
  const row = document.createElement("div");
  row.className = "audio-slider default-zoom-setting";
  const label = document.createElement("label");
  label.className = "audio-slider-label";
  label.textContent = "Default zoom";
  label.htmlFor = "default-zoom-slider";
  const control = document.createElement("div");
  control.className = "default-zoom-control";
  const input = document.createElement("input");
  input.id = "default-zoom-slider";
  input.type = "range";
  input.min = String(CAMERA.minZoom * 100);
  input.max = String(CAMERA.maxZoom * 100);
  input.step = "1";
  input.value = String(Math.round(readDefaultZoom() * 100));
  input.title = "Starting zoom for new games and Labs. Lab can still zoom in farther after starting.";
  const endpoints = document.createElement("div");
  endpoints.className = "default-zoom-endpoints";
  const out = document.createElement("span");
  out.textContent = "Zoomed out";
  const inside = document.createElement("span");
  inside.textContent = "Zoomed in";
  endpoints.append(out, inside);
  const sync = () => input.setAttribute("aria-valuetext", `${Number(input.value) / 100}× zoom`);
  input.addEventListener("input", () => {
    sync();
    try {
      globalThis.localStorage?.setItem(STORAGE_KEY, String(Number(input.value) / 100));
    } catch {
      // Keep the visible selection when storage is unavailable.
    }
  });
  sync();
  control.append(input, endpoints);
  row.append(label, control);
  root.appendChild(row);
}
