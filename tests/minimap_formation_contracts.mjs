import { _cancelFormationGesture } from "../client/src/input/formation_gesture.js";

export function runMinimapFormationContracts({ minimapHarness, lockedEvent, pointerEvent, listenerFor, recordingContext, assert }) {
  const selected = [
    { id: 7, owner: 1, kind: "rifleman", x: 10, y: 10 },
    { id: 8, owner: 1, kind: "rifleman", x: 20, y: 10 },
    { id: 9, owner: 2, kind: "rifleman", x: 30, y: 10 },
    { id: 10, owner: 1, kind: "scoutPlane", x: 40, y: 10 },
  ];
  for (const native of [false, true]) for (const attack of [false, true]) {
    const h = minimapHarness({ selected, commandTarget: attack ? "attack" : null,
      rect: { left: 100, top: 200, width: 121, height: 121, backingWidth: 242, backingHeight: 242 } });
    h.state.map.tileSize = 8;
    const button = attack ? 0 : 2;
    const event = (x, y) => native
      ? pointerEvent(h.canvas, x, y, { pointerType: "mouse", button, shiftKey: true })
      : lockedEvent(x, y, button, { shiftKey: true });
    const send = (phase, x, y) => native
      ? listenerFor(h.canvas, `pointer${phase}`)(event(x, y))
      : h.router[`pointer${phase[0].toUpperCase()}${phase.slice(1)}`](event(x, y));
    send("down", 110, 210);
    assert(h.net.sent.length === 0, "formation press waits for release");
    send("move", 140, 210);
    send("move", 140, 240);
    assert(h.centers.length === 0, "drawing a minimap line never pans the camera");
    _cancelFormationGesture.call({ _formationGesture: null, _intent: () => h.clientIntent });
    const preview = h.clientIntent.formationMovePreview;
    assert(preview?.points.length >= 3 && preview.slots.length === 2, "bent stroke previews only owned land-unit slots");
    h.minimap.ctx = recordingContext("formation");
    h.minimap._formation.draw();
    const calls = h.minimap.ctx.calls;
    assert(calls.some((c) => c.op === "stroke" && c.strokeStyle === (attack ? "#d47a5f" : "#72d5ea")),
      "minimap draws the command color");
    assert(calls.filter((c) => c.op === "arc").length === 2, "minimap draws both provisional slots");
    send("up", 160, 240);
    const command = h.net.sent[0];
    assert(h.net.sent.length === 1 && command.c === "formationMove", "one atomic formation command on release");
    assert(!!command.attackMove === attack && command.queued === true, "attack and Shift flags survive minimap gesture");
    assert(command.units.join() === "7,8", "enemy units and planes are excluded");
    assert(command.points[0].x === 160 && command.points[0].y === 160, "CSS-scaled minimap samples world coordinates");
    assert(command.points.some((p) => p.x === 640 && p.y === 160), "polyline retains its corner");
    assert(h.clientIntent.formationMovePreview === null, "release clears preview");
    if (native) assert(h.canvas.releasedPointers.includes(1), "native mouse capture is released");
    h.minimap.destroy();
  }
  for (const cancel of ["blur", "escape", "pointercancel", "source", "disabled", "target", "destroy"]) {
    const h = minimapHarness({ selected, commandTarget: "attack" });
    h.router.pointerDown(lockedEvent(110, 210));
    h.router.pointerMove(lockedEvent(160, 250));
    if (cancel === "blur") listenerFor(h.window, "blur")();
    if (cancel === "escape") listenerFor(h.window, "keydown")({ key: "Escape" });
    if (cancel === "pointercancel") listenerFor(h.canvas, "pointercancel")({ pointerId: 1 });
    if (cancel === "source") h.router.releaseSource("locked");
    if (cancel === "disabled") h.minimap.commandsEnabled = false;
    if (cancel === "target") h.clientIntent.endCommandTarget();
    if (cancel === "destroy") h.minimap.destroy();
    h.router.pointerUp(lockedEvent(180, 280));
    assert(h.net.sent.length === 0, `${cancel} never commits a formation`);
    assert(h.clientIntent.formationMovePreview === null, `${cancel} clears formation preview`);
    if (cancel !== "destroy") h.minimap.destroy();
  }
  {
    const h = minimapHarness({ selected });
    h.router.pointerDown(lockedEvent(120, 220, 2));
    for (let i = 0; i < 100; i++) h.router.pointerMove(lockedEvent(120 + i * 10, 230 + i * 10, 2));
    h.router.pointerUp(lockedEvent(2000, 2000, 2));
    const points = h.net.sent[0].points;
    assert(points.length <= 64, "minimap stroke respects the shared point cap");
    assert(points.every((p) => p.x >= 0 && p.x <= 241 && p.y >= 0 && p.y <= 241),
      "captured stroke stays inside world bounds");
    assert(!h.router.pointerMove(lockedEvent(2000, 2000, 2)), "release drops router capture outside minimap");
    h.minimap.destroy();
  }
  console.log("minimap_formation_contracts: ok");
}
