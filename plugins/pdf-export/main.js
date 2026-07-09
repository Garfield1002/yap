// PDF Export owns A4 mode and the print path. Page Break (a declared plugin
// requirement) owns all pagination widgets inside the editor.

export async function activate(yap) {
  const { StateEffect, StateField } = yap.cm.state;
  const { EditorView, ViewPlugin } = yap.cm.view;
  const settings = await yap.settings.get();
  let pageMode = settings.pageMode === true;
  const views = new Set();

  const setPageMode = StateEffect.define();
  const pageModeField = StateField.define({
    create: () => pageMode,
    update(value, transaction) {
      for (const effect of transaction.effects) {
        if (effect.is(setPageMode)) return effect.value;
      }
      return value;
    },
    provide: (field) =>
      EditorView.editorAttributes.from(field, (enabled) =>
        enabled ? { class: "yap-pdf-page-mode" } : {},
      ),
  });

  const tracker = ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.view = view;
        views.add(view);
      }
      destroy() {
        views.delete(this.view);
      }
    },
  );

  function togglePageMode() {
    pageMode = !pageMode;
    for (const view of views) view.dispatch({ effects: setPageMode.of(pageMode) });
    void yap.settings.set({ ...settings, pageMode });
  }

  function printPdf() {
    const view = views.values().next().value;
    if (!view) return;
    const dir = view.state.facet(yap.editor.documentDirectory);
    void yap.export.printMarkdown(view.state.doc.toString(), dir, {
      title: "yap document",
      renderLine: (line) =>
        /^\s*<!--\s*page-break\s*-->\s*$/.test(line)
          ? '<div class="yap-pdf-print-page-break"></div>'
          : undefined,
      css: `
        @page { size: A4; margin: 20mm; }
        body { color: #1f2328; font: 11pt/1.55 system-ui, sans-serif; }
        h1, h2, h3 { line-height: 1.2; margin: 1.2em 0 .45em; }
        p, li { margin: .45em 0; }
        pre { padding: .8em; overflow-wrap: anywhere; background: #f6f8fa; }
        code { font-family: ui-monospace, monospace; }
        img { max-width: 100%; }
        a { color: #0969da; }
        .yap-pdf-print-page-break { break-before: page; page-break-before: always; height: 0; }
      `,
    });
  }

  yap.editor.registerExtension([pageModeField, tracker]);
  yap.commands.register({
    id: "pdf-export.toggle-page-mode",
    title: "PDF: Toggle A4 Page Mode",
    keybinding: "Mod+Shift+P",
    run: togglePageMode,
  });
  yap.menus.addItem({ label: "Toggle A4 Page Mode", run: togglePageMode });
  yap.commands.register({ id: "pdf-export.print", title: "PDF: Print / Save as PDF…", run: printPdf });
  yap.menus.addItem({ label: "Print / Save as PDF…", run: printPdf });
  yap.statusBar.addItem({ id: "pdf-page-mode", text: "PDF: A4 mode", title: "Toggle A4 page boundaries", onClick: togglePageMode });
}
