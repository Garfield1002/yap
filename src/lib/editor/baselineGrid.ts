import { ViewPlugin, type EditorView } from "@codemirror/view";

/** The single vertical unit used by editor text, widgets, and paged surfaces. */
export const BASELINE_GRID_PX = 24;
export const FIRST_BASELINE_PX = BASELINE_GRID_PX * 2;

export type GridRounding = "nearest" | "up" | "down";

/** Snap a CSS-pixel measurement to the document's vertical grid. */
export function snapToBaselineGrid(value: number, rounding: GridRounding = "nearest"): number {
  if (!Number.isFinite(value)) return value;
  const units = value / BASELINE_GRID_PX;
  const rounded = rounding === "up" ? Math.ceil(units) : rounding === "down" ? Math.floor(units) : Math.round(units);
  return rounded * BASELINE_GRID_PX;
}

/** Heights always round up so a rendered or raw block never clips its content. */
export function gridAlignedHeight(requiredHeight: number, minimumHeight = BASELINE_GRID_PX): number {
  return Math.max(minimumHeight, snapToBaselineGrid(requiredHeight, "up"));
}

export interface BaselineMeasurements {
  body: number;
  h1: number;
  h2: number;
  h3: number;
  h4: number;
  h5: number;
  h6: number;
}

export interface BaselineCalibration {
  contentPaddingTop: number;
  /** Distance from a line box's top to its grid-aligned text baseline. */
  blockInset: number;
  headingShifts: readonly [number, number, number, number, number, number];
}

/** Calculate offsets from actual DOM baselines, all relative to body phase. */
export function calculateBaselineCalibration(m: BaselineMeasurements): BaselineCalibration {
  const body = Number.isFinite(m.body) && m.body > 0 ? m.body : 18;

  return {
    contentPaddingTop: Math.max(0, FIRST_BASELINE_PX - body),
    blockInset: body,
    // Match the body's exact offset within the line box. Since every line box
    // is a grid multiple, this puts even a document-opening heading at 48px.
    headingShifts: [m.h1, m.h2, m.h3, m.h4, m.h5, m.h6].map((baseline) => body - baseline) as unknown as readonly [
      number,
      number,
      number,
      number,
      number,
      number,
    ],
  };
}

let resolveFirstCalibration!: () => void;
let firstCalibrationDone = false;
const firstCalibration = new Promise<void>((resolve) => {
  resolveFirstCalibration = resolve;
});

/** Resolves after the first editor has calibrated against its loaded fonts. */
export function waitForBaselineCalibration(): Promise<void> {
  return firstCalibration;
}

function announceReady(view: EditorView): void {
  view.dom.dataset.baselineGridReady = "true";
  if (!firstCalibrationDone) {
    firstCalibrationDone = true;
    resolveFirstCalibration();
  }
  document.dispatchEvent(new CustomEvent("bulletmd:baseline-grid-ready", { detail: { view } }));
}

const headingClasses = ["", "cm-h1", "cm-h2", "cm-h3", "cm-h4", "cm-h5", "cm-h6"];

function createProbe(view: EditorView): HTMLElement {
  const root = document.createElement("div");
  root.className = "cm-content bulletmd-baseline-probes";
  root.setAttribute("aria-hidden", "true");
  // Always measure uncorrected geometry when recalibrating.
  for (let level = 1; level <= 6; level++) root.style.setProperty(`--baseline-h${level}-shift`, "0px");

  for (let level = 0; level <= 6; level++) {
    const line = document.createElement("div");
    line.className = `cm-line ${headingClasses[level]}`.trim();
    line.dataset.probeLevel = String(level);
    line.append("Hamburgefontsiv ");
    const marker = document.createElement("span");
    marker.className = "bulletmd-baseline-marker";
    line.append(marker);
    root.append(line);
  }
  view.dom.append(root);
  return root;
}

function readBaselines(probe: HTMLElement): BaselineMeasurements | null {
  const values: number[] = [];
  for (let level = 0; level <= 6; level++) {
    const line = probe.querySelector<HTMLElement>(`[data-probe-level="${level}"]`);
    const marker = line?.querySelector<HTMLElement>(".bulletmd-baseline-marker");
    if (!line || !marker) return null;
    values.push(marker.getBoundingClientRect().top - line.getBoundingClientRect().top);
  }
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) return null;
  return { body: values[0], h1: values[1], h2: values[2], h3: values[3], h4: values[4], h5: values[5], h6: values[6] };
}

class BaselineGridController {
  private destroyed = false;
  private scheduled = false;
  private readonly resizeObserver: ResizeObserver | undefined;
  private readonly mutationObserver: MutationObserver;
  private readonly scheduleBound = () => this.schedule();

  constructor(private readonly view: EditorView) {
    this.resizeObserver = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(this.scheduleBound);
    this.resizeObserver?.observe(view.dom);
    window.addEventListener("resize", this.scheduleBound);
    window.visualViewport?.addEventListener("resize", this.scheduleBound);
    document.fonts?.addEventListener("loadingdone", this.scheduleBound);
    this.mutationObserver = new MutationObserver(this.scheduleBound);
    // Theme changes are rare; opacity writes also touch the root style but do
    // not affect font metrics and must not recalibrate on every slider step.
    this.mutationObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });

    const fontsReady = document.fonts?.ready ?? Promise.resolve();
    void fontsReady.then(this.scheduleBound, this.scheduleBound);
  }

  private schedule(): void {
    if (this.destroyed || this.scheduled) return;
    this.scheduled = true;
    requestAnimationFrame(() => {
      this.scheduled = false;
      if (this.destroyed) return;
      const probe = createProbe(this.view);
      this.view.requestMeasure({
        read: () => readBaselines(probe),
        write: (measurements) => {
          probe.remove();
          if (!measurements || this.destroyed) return;
          const calibration = calculateBaselineCalibration(measurements);
          const style = this.view.dom.style;
          style.setProperty("--baseline-content-pad-top", `${calibration.contentPaddingTop}px`);
          style.setProperty("--baseline-block-inset", `${calibration.blockInset}px`);
          calibration.headingShifts.forEach((shift, index) => style.setProperty(`--baseline-h${index + 1}-shift`, `${shift}px`));
          // The CSS write changes content padding and some line geometry after
          // this measure's read phase. Queue one clean CM measurement so its
          // height map observes the calibrated values as a single update.
          this.view.requestMeasure({
            read: () => null,
            // Readiness means CodeMirror has completed a measurement using the
            // calibrated CSS, not merely that those CSS variables were written.
            write: () => announceReady(this.view),
          });
        },
      });
    });
  }

  destroy(): void {
    this.destroyed = true;
    this.resizeObserver?.disconnect();
    this.mutationObserver.disconnect();
    window.removeEventListener("resize", this.scheduleBound);
    window.visualViewport?.removeEventListener("resize", this.scheduleBound);
    document.fonts?.removeEventListener("loadingdone", this.scheduleBound);
  }
}

/** Runtime font calibration and safe CodeMirror remeasurement. */
export const baselineGrid = ViewPlugin.fromClass(BaselineGridController);
