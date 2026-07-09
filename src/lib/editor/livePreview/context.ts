import { Facet } from "@codemirror/state";

/**
 * Directory of the file being edited. Relative image paths resolve against it,
 * so the editor must be told where the document lives.
 */
export const documentDirectory = Facet.define<string, string>({
  combine: (values) => values[0] ?? "",
});
