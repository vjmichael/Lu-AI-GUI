// R23 model-specific inference settings. The Rust host validates again.
export const DEFAULT_PROFILE = Object.freeze({
  contextTokens: 4096, maxNewTokens: 512, temperature: 0.0,
  topP: 0.95, topK: 20, minP: 0, threads: 4
});
export function normalizeProfile(input) {
  const p = {...DEFAULT_PROFILE, ...(input || {})};
  const int = (value, lo, hi) => Number.isInteger(value) && value >= lo && value <= hi;
  const float = (value, lo, hi) => Number.isFinite(value) && value >= lo && value <= hi;
  if (!int(p.contextTokens, 4096, 262144) || p.contextTokens % 1024 !== 0 ||
      !int(p.maxNewTokens, 512, 4096) || p.maxNewTokens >= p.contextTokens ||
      !float(p.temperature, 0, 2) || !float(p.topP, 0.01, 1) ||
      !int(p.topK, 1, 100) || !float(p.minP, 0, 1) || !int(p.threads, 1, 64)) {
    throw Error('Invalid inference profile; review limits and sampling values');
  }
  return p;
}
export function profileField(profile, key, value) {
  const raw = Number(value);
  return {...profile, [key]:raw};
}
