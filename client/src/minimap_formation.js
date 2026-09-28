import { isUnit, KIND } from "./protocol.js";
import { ownOwner } from "./minimap_targeting.js";

/** Mouse/locked-cursor formation gestures; touch and pen retain tap/pan semantics. */
export class MinimapFormation {
  constructor(minimap, createGesture) {
    this.minimap = minimap;
    this.session = null;
    const unitIds = () => (minimap.state.selectedEntities() || [])
      .filter((e) => ownOwner(minimap.state, e.owner, minimap.controlPolicy) &&
        isUnit(e.kind) && e.kind !== KIND.SCOUT_PLANE).map((e) => e.id);
    this.unitIds = unitIds;
    this.gesture = createGesture?.({
      state: minimap.state,
      commandInteraction: minimap.commandInteraction,
      _groundAtScreen: (x, y) => ({ x, y }),
      _selectedOwnUnitIds: unitIds,
      _selectedOwnLandUnitIds: unitIds,
      _intent: () => minimap._intent(),
      _onRightClick: (p, ev) => minimap._issueOrder(p.x, p.y, !!ev.shiftKey),
      _issueTargetedCommand: (p, ev) => minimap._issueOrder(p.x, p.y, !!ev.shiftKey),
      _addCommandFeedback: (...args) => minimap._addCommandFeedback(...args),
    });
    this.onKeyDown = (ev) => { if (ev.key === "Escape") this.cancel(); };
    window.addEventListener("keydown", this.onKeyDown);
  }

  point(ev) {
    const m = this.minimap;
    if (!m._ensureTransform()) return null;
    const p = m._eventToCanvas(ev);
    return m._canvasToWorld(p.x, p.y);
  }

  down(ev) {
    if (this.session) { this.cancel(); return true; }
    const native = ev.source !== "locked";
    if (native && ev.pointerType !== "mouse") return false;
    const m = this.minimap;
    const intent = m._intent();
    const target = intent?.commandTarget;
    const kind = ev.button === 2 && !target ? "move"
      : ev.button === 0 && target === "attack" ? "attackMove" : null;
    if (!this.gesture || !kind || !m._commandsEnabled() || intent?.placement ||
        intent?.activeLabTool || !this.unitIds().length) return false;
    const p = this.point(ev);
    if (!p) return false;
    if (native) m.inputRouter?.pointerMove(m._routerEvent(ev, "dom"));
    this.session = { button: ev.button, pointerId: native ? ev.pointerId : null, target };
    this.gesture.begin(p, ev, kind);
    if (native) m._capturePointer(ev.pointerId);
    ev.preventDefault?.();
    ev.stopPropagation?.();
    ev.originalEvent?.preventDefault?.();
    ev.originalEvent?.stopPropagation?.();
    return true;
  }

  valid() {
    const m = this.minimap;
    const intent = m._intent();
    return !this.session.cancelled && m._commandsEnabled() &&
      intent?.commandTarget === this.session.target && !intent?.placement && !intent?.activeLabTool;
  }

  move(ev) {
    if (!this.session) return false;
    if (this.session.pointerId != null && ev.pointerId !== this.session.pointerId) return true;
    if (!this.valid()) this.cancel();
    const p = this.point(ev);
    if (!this.session.cancelled && p) this.gesture.update(p, ev);
    ev.preventDefault?.();
    return true;
  }

  up(ev) {
    if (!this.session) return false;
    if (this.session.pointerId != null && ev.pointerId !== this.session.pointerId) return true;
    const p = this.point(ev);
    // A chord can release capture with a different button. End that session too,
    // but only the initiating button may commit the order.
    if (ev.button === this.session.button && this.valid() && p) this.gesture.finish(p, ev);
    else this.gesture.cancel();
    this.release();
    if (!this.minimap._containsClientPoint(ev.clientX, ev.clientY)) {
      this.minimap.inputRouter?.releaseSource?.("dom");
    }
    ev.preventDefault?.();
    return true;
  }

  cancel() {
    if (!this.session) return;
    this.gesture.cancel();
    this.session.cancelled = true;
  }

  release() {
    if (this.session?.pointerId != null) this.minimap._releasePointer(this.session.pointerId);
    this.session = null;
  }

  reset() { this.cancel(); this.release(); }

  draw() {
    if (!this.session || this.session.cancelled) return;
    if (!this.valid()) { this.cancel(); return; }
    const m = this.minimap;
    const preview = m._intent()?.formationMovePreview;
    if (!preview?.points?.length) return;
    const ctx = m.ctx;
    const scale = m._presentationScale();
    ctx.save();
    ctx.beginPath();
    preview.points.forEach((p, index) => {
      const c = m._worldToCanvas(p.x, p.y);
      if (index === 0) ctx.moveTo(c.x, c.y);
      else ctx.lineTo(c.x, c.y);
    });
    ctx.strokeStyle = "#071018";
    ctx.lineWidth = 5 * scale;
    ctx.stroke();
    ctx.strokeStyle = preview.kind === "attackMove" ? "#d47a5f" : "#72d5ea";
    ctx.lineWidth = 2 * scale;
    ctx.stroke();
    for (const slot of preview.slots || []) {
      const c = m._worldToCanvas(slot.x, slot.y);
      ctx.beginPath();
      ctx.arc(c.x, c.y, Math.max(2 * scale, slot.radius * m._scale), 0, Math.PI * 2);
      ctx.stroke();
    }
    ctx.restore();
  }

  destroy() {
    this.reset();
    window.removeEventListener("keydown", this.onKeyDown);
  }
}
