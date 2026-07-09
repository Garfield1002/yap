import type { EditorState, Range } from "@codemirror/state";
import { Decoration, type DecorationSet, type WidgetType } from "@codemirror/view";
import { overlapsRegion, type Region } from "./activeRegions";

export interface Decorations {
  /** Everything the view renders. */
  deco: DecorationSet;
  /**
   * Only the replace/widget ranges. `EditorView.atomicRanges` makes the cursor
   * skip over these; feeding it mark decorations would make arrow keys jump
   * across styled words.
   */
  atomic: DecorationSet;
}

const HIDDEN = Decoration.replace({});

/**
 * Accumulates decoration ranges for one document scan.
 *
 * The central rule: `hide` and `replace` are suppressed inside a raw region,
 * but `mark` and `line` are not. That is what makes revealing a block feel
 * still — markup characters appear, but nothing about the text restyles or
 * shifts.
 */
export class Builder {
  private readonly ranges: Range<Decoration>[] = [];
  private readonly atomicRanges: Range<Decoration>[] = [];

  constructor(
    readonly state: EditorState,
    readonly regions: Region[],
  ) {}

  /** True when this span is inside a block that is currently showing source. */
  isRaw(from: number, to: number): boolean {
    return overlapsRegion(this.regions, from, to);
  }

  /** Collapse a run of markup characters. No-op while raw. */
  hide(from: number, to: number): void {
    if (from >= to || this.isRaw(from, to)) return;
    this.ranges.push(HIDDEN.range(from, to));
    this.atomicRanges.push(HIDDEN.range(from, to));
  }

  /**
   * Swap a whole node for a rendered widget. No-op while raw.
   *
   * `atomic` (the default) makes the cursor skip the widget, which is right for
   * an inline atom like a rendered image or inline formula. A block that should
   * behave like fenced code -- where an arrow key lands *on* the block and
   * reveals its source rather than jumping past it -- passes `atomic: false`.
   */
  replace(from: number, to: number, widget: WidgetType, block = false, atomic = true): void {
    if (from > to || this.isRaw(from, to)) return;
    const deco = Decoration.replace({ widget, block });
    this.ranges.push(deco.range(from, to));
    if (atomic) this.atomicRanges.push(deco.range(from, to));
  }

  /** Style a span. Always applied, raw or not. */
  mark(from: number, to: number, className: string): void {
    if (from >= to) return;
    this.ranges.push(Decoration.mark({ class: className }).range(from, to));
  }

  /** Style the line containing `pos`. Always applied, raw or not. */
  line(pos: number, className: string): void {
    const start = this.state.doc.lineAt(pos).from;
    this.ranges.push(Decoration.line({ class: className }).range(start));
  }

  finish(): Decorations {
    // `Decoration.set(_, true)` sorts. A RangeSetBuilder would throw here:
    // nested constructs (bold inside a link inside a list) emit out of order.
    return {
      deco: Decoration.set(this.ranges, true),
      atomic: Decoration.set(this.atomicRanges, true),
    };
  }
}
