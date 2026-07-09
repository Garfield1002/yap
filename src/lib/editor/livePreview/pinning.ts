import type { ChangeDesc, EditorSelection, Range } from "@codemirror/state";
import type { Decoration, DecorationSet } from "@codemirror/view";
import type { Region } from "./activeRegions";

/**
 * Move a pinned region through an edit without letting it grow at its edges.
 *
 * `from` maps to *after* text inserted at the boundary and `to` to *before* it,
 * so typing at either end of a block leaves the block rather than stretching it.
 * Without that, a cursor sitting at the end of a paragraph would extend the
 * pinned region with every character and the whole document would eventually be
 * pinned raw.
 */
export function mapRegions(regions: Region[], changes: ChangeDesc): Region[] {
  return regions.map((r) => ({
    from: changes.mapPos(r.from, 1),
    to: changes.mapPos(r.to, -1),
  }));
}

/** True when [from, to] lies within one of the regions. */
export function contains(regions: Region[], from: number, to: number): boolean {
  return regions.some((r) => from >= r.from && to <= r.to);
}

/** Every edit in this transaction happened inside a pinned block. */
export function changesStayInside(regions: Region[], changes: ChangeDesc): boolean {
  let inside = true;
  // Old-document coordinates, which is what the un-mapped regions are in.
  changes.iterChangedRanges((fromA, toA) => {
    if (!contains(regions, fromA, toA)) inside = false;
  });
  return inside;
}

/** Every cursor is still inside a pinned block. */
export function selectionStaysInside(regions: Region[], selection: EditorSelection): boolean {
  return selection.ranges.every((r) => contains(regions, r.from, r.to));
}

/** Collect the ranges of `set` that a predicate keeps. */
function collect(
  set: DecorationSet,
  keep: (from: number, to: number) => boolean,
): Range<Decoration>[] {
  const out: Range<Decoration>[] = [];
  for (const iter = set.iter(); iter.value; iter.next()) {
    if (keep(iter.from, iter.to)) out.push(iter.value.range(iter.from, iter.to));
  }
  return out;
}

export const inside = (set: DecorationSet, regions: Region[]) =>
  collect(set, (from, to) => contains(regions, from, to));

export const outside = (set: DecorationSet, regions: Region[]) =>
  collect(set, (from, to) => !contains(regions, from, to));
