// Entitlement — single paywall checkpoint (AGENTS.md §10).
// Default tier=free, offline=true. No network calls. No DB.
// Future: verify signed JWT (Ed25519) from Stripe/LemonSqueezy, 30-day grace.

/** @typedef {"free"|"pro"|"team"} Tier */
/** @typedef {"wifi"|"usb"|"1080p60"|"multiMonitor"|"touch"|"clipboard"|"nvenc"} Feature */

export const TIERS = ["free", "pro", "team"];

/** @type {Record<Feature, Tier>} minimum tier required per feature */
export const FEATURE_MIN_TIER = {
  wifi: "free",
  usb: "pro",          // usb-tethering + adb-reverse
  "1080p60": "pro",
  multiMonitor: "pro",
  touch: "pro",
  clipboard: "pro",
  nvenc: "pro",
};

const RANK = { free: 0, pro: 1, team: 2 };

/**
 * @param {Feature} feature
 * @param {Tier} tier
 * @returns {boolean}
 */
export function canUse(feature, tier = "free") {
  if (!(feature in FEATURE_MIN_TIER)) return false;
  if (!RANK.hasOwnProperty(tier)) return false;
  return RANK[tier] >= RANK[FEATURE_MIN_TIER[feature]];
}

/**
 * Stub validator — accepts empty key as free. Real JWT verify plugs in here.
 * @param {{key?: string, tier?: Tier, device_id?: string}} license
 * @returns {{tier: Tier, offline: boolean, grace: boolean}}
 */
export function validateLicense(license = {}) {
  const tier = TIERS.includes(license.tier) ? license.tier : "free";
  return { tier, offline: true, grace: false };
}

/** Free-tier video ceiling enforced in one place (UI calls this, never hardcodes). */
export function maxVideoForTier(tier = "free") {
  if (tier === "free") return { width: 1280, height: 720, fps: 30 };
  return { width: 1920, height: 1080, fps: 60 };
}
