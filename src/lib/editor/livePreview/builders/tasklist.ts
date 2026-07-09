import type { SyntaxNodeRef } from "@lezer/common";
import type { Builder } from "../builder";
import { CheckboxWidget } from "../widgets/CheckboxWidget";

/** `[ ]` / `[x]` at the head of a list item becomes a clickable checkbox. */
export function tasklist(node: SyntaxNodeRef, b: Builder): boolean | void {
  if (node.name !== "TaskMarker") return;

  const checked = b.state.doc.sliceString(node.from + 1, node.to - 1).toLowerCase() === "x";
  b.replace(node.from, node.to, new CheckboxWidget(checked));
  if (checked) b.line(node.from, "cm-task-done");
  return false;
}
