import { assert } from "./assertions.mjs";
import { createOrthographicProjectionSnapshot } from "../../client/src/camera_projection.js";
import { Input } from "../../client/src/input/index.js";
import { buildSelectionScene } from "../../client/src/input/selection_projection.js";
import { KIND } from "../../client/src/protocol.js";

import { ClientIntent } from "../../client/src/client_intent.js";
import { buildRendererFeedbackView } from "../../client/src/renderer/feedback_view_model.js";
import { STATS } from "../../client/src/config.js";

const worker = { id: 1, owner: 1, kind: KIND.WORKER, x: 64, y: 64 };
const friendlyTank = { id: 2, owner: 2, kind: KIND.TANK, x: 112, y: 112, facing: 0 };
const oil = { id: 3, owner: 0, kind: KIND.OIL, x: 112, y: 112, remaining: 1000 };
const map = { width: 64, height: 64, tileSize: 32 };
const input = Object.create(Input.prototype);
const commands = [];
input.state = {
  playerId: 1,
  map,
  entitiesInterpolated: () => [worker, friendlyTank, oil],
  selectedEntities: () => [worker],
  isAllyOwner: (owner) => owner === 2,
  isEnemyOwner: () => false,
  addCommandFeedback() {},
};
input.clientIntent = new ClientIntent();
input.commandInteraction = { issueCommand(command) { commands.push(command); } };
input._groundAtScreen = (x, y) => ({ x, y });
const projection = createOrthographicProjectionSnapshot({
  x: 0,
  y: 0,
  zoom: 1,
  worldW: map.width * map.tileSize,
  worldH: map.height * map.tileSize,
  viewW: 640,
  viewH: 480,
});
input.selectionScene = buildSelectionScene({
  entities: input.state.entitiesInterpolated(),
  tileSize: map.tileSize,
  projection,
});

const friendlyTankHullPoint = { x: friendlyTank.x + 24, y: friendlyTank.y };
assert(
  input._resourceAtScreen(friendlyTankHullPoint) === null &&
    input._entityAtScreen(friendlyTankHullPoint)?.id === friendlyTank.id,
  "Pump Jack friendly-unit fixture must hit the tank hull outside the oil proxy",
);

input.mouse = friendlyTankHullPoint;
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "nearby move has no build footprint");
input._onRightClick(friendlyTankHullPoint);
assert(
  commands.length === 1 &&
    commands[0].c === "move" &&
    commands[0].units.join(",") === String(worker.id),
  "Engineer right-click outside the oil patch remains a move",
);

commands.length = 0;
input._onRightClick({ x: oil.x, y: oil.y });
assert(commands.length === 1 && commands[0].c === "build" &&
  commands[0].building === KIND.PUMP_JACK && commands[0].tileX === 3 && commands[0].tileY === 3,
  "Engineer right-click on live oil builds a centered Pump Jack");

input.mouse = { x: oil.x, y: oil.y };
input._refreshAttackTargetPreview();
const hover = input.clientIntent.contextualBuildPreview;
assert(hover?.building === commands[0].building && hover.tileX === commands[0].tileX &&
  hover.tileY === commands[0].tileY && hover.valid, "hover footprint matches the actual build command");
assert(input.clientIntent.placement === null, "hover does not arm left-click placement");
assert(STATS[hover.building].footW === 1 && STATS[hover.building].footH === 1,
  "contextual Pump Jack footprint is one by one tile");
const feedback = (options = {}) => buildRendererFeedbackView(input.state,
  { clientIntent: input.clientIntent, ...options });
assert(feedback().placement === hover, "renderer receives the contextual footprint");
assert(feedback({ previewSurface: "minimap" }).placement === null, "UI hover hides the footprint");
input.clientIntent.beginCommandTarget("move");
assert(feedback().placement === null, "explicit move mode hides the contextual footprint immediately");
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "explicit command clears build hover");
input.clientIntent.endCommandTarget();
input._refreshAttackTargetPreview();
input._drag = {};
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "selection drag clears build hover");
input._drag = null;
input._refreshAttackTargetPreview();
input.mouse = friendlyTankHullPoint;
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "moving off oil clears a previous footprint");
input.mouse = { x: oil.x, y: oil.y };
input.state.selectedEntities = () => [friendlyTank];
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "selection without an Engineer has no footprint");
input.state.selectedEntities = () => [worker];
oil.remaining = 0;
input.selectionScene = buildSelectionScene({ entities: [worker, friendlyTank, oil], tileSize: map.tileSize, projection });
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null, "depleted oil has no footprint");
oil.remaining = 1000;

const attackingTank = { id: 60, owner: 1, kind: KIND.TANK, x: 64, y: 64, facing: 0 };
const enemyPumpJack = { id: 61, owner: 3, kind: KIND.PUMP_JACK, x: oil.x, y: oil.y, hp: 100, maxHp: 100 };
input.state.selectedEntities = () => [attackingTank];
input.state.isEnemyOwner = (owner) => owner === 3;
input.selectionScene = buildSelectionScene({
  entities: [attackingTank, oil, enemyPumpJack],
  tileSize: map.tileSize,
  projection,
});
commands.length = 0;
input._onRightClick({ x: oil.x, y: oil.y });
assert(
  commands.length === 1 && commands[0].c === "attack" && commands[0].target === enemyPumpJack.id,
  "enemy Pump Jack wins contextual attack targeting over its underlying oil patch",
);

commands.length = 0;
input._onRightClick({ x: oil.x + map.tileSize / 2 + map.tileSize / 6 - 0.1, y: oil.y });
assert(
  commands.length === 1 && commands[0].c === "attack" && commands[0].target === enemyPumpJack.id,
  "enemy Pump Jack accepts a contextual attack within one-sixth tile beyond its footprint",
);

input.state.selectedEntities = () => [worker];
input._refreshAttackTargetPreview();
assert(input.clientIntent.contextualBuildPreview === null && input.clientIntent.attackTargetPreview,
  "enemy extractor attack takes precedence over Engineer build hover");
input.state.selectedEntities = () => [attackingTank];

const steel = { id: 62, owner: 0, kind: KIND.STEEL, x: 176, y: 176, remaining: 1000 };
const enemySteelMine = { id: 63, owner: 3, kind: KIND.STEEL_MINE, x: steel.x, y: steel.y, hp: 100, maxHp: 100 };
input.selectionScene = buildSelectionScene({
  entities: [attackingTank, steel, enemySteelMine],
  tileSize: map.tileSize,
  projection,
});
commands.length = 0;
input._onRightClick({ x: steel.x, y: steel.y });
assert(
  commands.length === 1 && commands[0].c === "attack" && commands[0].target === enemySteelMine.id,
  "enemy Steel Mine wins contextual attack targeting over its underlying steel patch",
);

commands.length = 0;
input._onRightClick({ x: steel.x + map.tileSize / 2 + map.tileSize / 6 - 0.1, y: steel.y });
assert(
  commands.length === 1 && commands[0].c === "attack" && commands[0].target === enemySteelMine.id,
  "enemy Steel Mine accepts a contextual attack within one-sixth tile beyond its footprint",
);

const steelMineScaffold = {
  id: 4,
  owner: 1,
  kind: KIND.STEEL_MINE,
  x: 144,
  y: 144,
  buildProgress: 0.5,
};
input.state.entitiesInterpolated = () => [worker, steelMineScaffold];
input.selectionScene = buildSelectionScene({
  entities: input.state.entitiesInterpolated(),
  tileSize: map.tileSize,
  projection,
});
commands.length = 0;
input._onRightClick({ x: steelMineScaffold.x, y: steelMineScaffold.y });
assert(
  commands.length === 1 && commands[0].c === "move",
  "Engineer right-click on a depot-built extractor scaffold moves instead of issuing an invalid build command",
);

const miningAnchorInput = Object.create(Input.prototype);
miningAnchorInput.state = {
  playerId: 1,
  isOwnOwner: (owner) => owner === 1,
  isAllyOwner: (owner) => owner === 2,
};
miningAnchorInput._selectionEntities = () => [
  { id: 40, owner: 3, kind: KIND.RESOURCE_DEPOT, x: 1, y: 1, buildProgress: null },
  { id: 41, owner: 2, kind: KIND.RESOURCE_DEPOT, x: 8, y: 8, buildProgress: 0.5 },
  { id: 42, owner: 2, kind: KIND.RESOURCE_DEPOT, x: 20, y: 20, buildProgress: null },
  { id: 43, owner: 1, kind: KIND.RESOURCE_DEPOT, x: 40, y: 40, buildProgress: null },
];
assert(
  miningAnchorInput._nearestCompletedMiningAnchor(0, 0, true)?.id === 42,
  "resource-range preview should use the nearest completed owned or allied mining anchor",
);
assert(
  miningAnchorInput._nearestCompletedMiningAnchor(0, 0)?.id === 43,
  "steel resource-range preview should continue to require an owned mining anchor",
);

const snapInput = Object.create(Input.prototype);
const nearOil = { id: 50, owner: 0, kind: KIND.OIL, x: 176, y: 176, remaining: 962 };
const farOil = { id: 51, owner: 0, kind: KIND.OIL, x: 336, y: 336, remaining: 962 };
const depletedOil = { id: 52, owner: 0, kind: KIND.OIL, x: 161, y: 161, remaining: 0 };
let placementPreview = null;
snapInput.state = { map };
snapInput.mouse = { x: 160, y: 160 };
snapInput._groundAtScreen = () => ({ x: 160, y: 160 });
snapInput._selectionEntities = () => [nearOil, farOil, depletedOil];
snapInput._footprintValid = (tileX, tileY) => tileX === 5 && tileY === 5;
snapInput.clientIntent = {
  placement: { building: KIND.PUMP_JACK, tileX: 0, tileY: 0, valid: false },
  updatePlacement(tileX, tileY, valid) {
    placementPreview = { tileX, tileY, valid };
  },
};
snapInput._refreshPlacement();
assert(
  placementPreview?.tileX === 5 && placementPreview?.tileY === 5 && placementPreview?.valid,
  "armed Pump Jack placement snaps to the closest nearby visible live oil patch",
);

snapInput._groundAtScreen = () => ({ x: 32, y: 32 });
snapInput._refreshPlacement();
assert(
  placementPreview?.tileX === 1 && placementPreview?.tileY === 1 && !placementPreview?.valid,
  "armed Pump Jack placement does not target an oil patch far outside the cursor area",
);

console.log("pump_jack_input_contracts: ok");
