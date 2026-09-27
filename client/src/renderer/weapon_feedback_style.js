import { KIND, WEAPON_KIND } from "../protocol.js";
import { muzzleFlashRadius } from "./shared.js";

const TANK_COAX_FLASH_RADIUS = 6;
const TANK_COAX_TRACER_COLOR = 0xfff0a6;
const TANK_COAX_TRACER_CORE_COLOR = 0xffffff;
const RIFLE_TRACER_WIDTH_SCALE = 0.3;
const MACHINE_GUN_TRACER_WIDTH_SCALE = 0.5;

export function muzzleFeedbackStyle(feedbackKind, weaponKind) {
  const widthScale = tracerWidthScale(feedbackKind, weaponKind);
  if (weaponKind === WEAPON_KIND.TANK_COAX) {
    return {
      flashRadius: TANK_COAX_FLASH_RADIUS,
      tracerWidth: 1.8 * widthScale,
      tracerColor: TANK_COAX_TRACER_COLOR,
      tracerAlpha: 0.98,
      tracerCoreWidth: 0.75 * widthScale,
      tracerCoreColor: TANK_COAX_TRACER_CORE_COLOR,
      tracerCoreAlpha: 0.72,
    };
  }
  return {
    flashRadius: muzzleFlashRadius(feedbackKind),
    tracerWidth: (feedbackKind === KIND.ANTI_TANK_GUN ? 2.5 : 1.5) * widthScale,
    tracerColor: 0xffe066,
    tracerAlpha: 0.92,
    tracerCoreWidth: 0,
    tracerCoreColor: 0xffffff,
    tracerCoreAlpha: 0,
  };
}

function tracerWidthScale(feedbackKind, weaponKind) {
  if (weaponKind === WEAPON_KIND.RIFLEMAN_RIFLE || feedbackKind === KIND.RIFLEMAN) {
    return RIFLE_TRACER_WIDTH_SCALE;
  }
  if (
    weaponKind === WEAPON_KIND.MACHINE_GUNNER_MG ||
    weaponKind === WEAPON_KIND.SCOUT_CAR_MG ||
    weaponKind === WEAPON_KIND.TANK_COAX ||
    feedbackKind === KIND.MACHINE_GUNNER ||
    feedbackKind === KIND.SCOUT_CAR
  ) {
    return MACHINE_GUN_TRACER_WIDTH_SCALE;
  }
  return 1;
}
