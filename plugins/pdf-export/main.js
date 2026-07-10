// PDF Export owns A4 mode and the print path. Page Break (a declared plugin
// requirement) owns all pagination widgets inside the editor.

export async function activate(bulletmd) {
  const { StateEffect, StateField } = bulletmd.cm.state;
  const { EditorView, ViewPlugin } = bulletmd.cm.view;
  const settings = await bulletmd.settings.get();
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
        enabled ? { class: "bulletmd-pdf-page-mode" } : {},
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
    void bulletmd.settings.set({ ...settings, pageMode });
  }

  function printPdf() {
    const view = views.values().next().value;
    if (!view) return;
    const dir = view.state.facet(bulletmd.editor.documentDirectory);
    const rootStyle = getComputedStyle(document.documentElement);
    const viewStyle = getComputedStyle(view.dom);
    const opacity = rootStyle.getPropertyValue("--dot-opacity").trim() || "12%";
    const topPadding = viewStyle.getPropertyValue("--baseline-content-pad-top").trim() || "30px";
    const blockInset = Number.parseFloat(
      viewStyle.getPropertyValue("--baseline-block-inset").trim(),
    ) || 18;
    const headingShift = (level) =>
      viewStyle.getPropertyValue(`--baseline-h${level}-shift`).trim() || "0px";
    void bulletmd.export.printMarkdown(view.state.doc.toString(), dir, {
      title: "bulletmd document",
      snapImagesToGrid: 24,
      layoutWidth: "170mm",
      blockInset,
      renderLine: (line) =>
        /^\s*<!--\s*page-break\s*-->\s*$/.test(line)
          ? '<div class="bulletmd-pdf-print-page-break"></div>'
          : undefined,
      css: `
        @page {
          size: A4;
          margin: ${topPadding} 20mm 48px;
          background-color: #f7f7f5;
          background-image: radial-gradient(circle at 0 0, color-mix(in srgb, #171717 ${opacity}, transparent) 1px, transparent 1.15px);
          background-size: 24px 24px;
        }
        html { color: #171717; background: #f7f7f5; }
        body {
          margin: 0;
          color: #171717;
          font: 16px/24px "Inter", "Segoe UI", system-ui, sans-serif;
          background-color: #f7f7f5;
        }
        h1 { min-height: 72px; padding-bottom: 24px; font-size: 1.9em; line-height: 48px; transform: translateY(${headingShift(1)}); }
        h2 { min-height: 48px; font-size: 1.5em; line-height: 48px; transform: translateY(${headingShift(2)}); }
        h3 { min-height: 48px; font-size: 1.25em; line-height: 48px; transform: translateY(${headingShift(3)}); }
        h4 { transform: translateY(${headingShift(4)}); }
        h5 { transform: translateY(${headingShift(5)}); }
        h6 { transform: translateY(${headingShift(6)}); }
        h1, h2, h3, h4, h5, h6, p, ul, ol, li, blockquote { box-sizing: border-box; margin: 0; }
        h4, h5, h6, p, li { min-height: 24px; line-height: 24px; }
        pre { margin: 0; padding: 24px; overflow-wrap: anywhere; background: #efefec; line-height: 24px; break-inside: avoid; }
        code { font-family: ui-monospace, monospace; }
        img { display: block; max-width: 100%; height: auto; break-inside: avoid; background: #f7f7f5; }
        blockquote { margin: 0; padding: 0 24px; break-inside: avoid; background: #f7f7f5; }
        table { margin: 0; border-collapse: collapse; break-inside: avoid; background: #f7f7f5; }
        th, td { box-sizing: border-box; height: 24px; padding: 0 6px; line-height: 24px; }
        .katex-display { margin: 0; }
        .bulletmd-print-grid-block { position: relative; display: block; break-inside: avoid; background: transparent; }
        .bulletmd-print-grid-surface { position: absolute; left: 0; right: 0; overflow: hidden; background: #f7f7f5; }
        .bulletmd-print-grid-surface > .katex-display { display: flex; align-items: flex-start; min-height: 100%; }
        a { color: #0969da; }
        .bulletmd-pdf-print-page-break { break-before: page; page-break-before: always; height: 0; }
      `,
    });
  }

  bulletmd.editor.registerExtension([pageModeField, tracker]);
  bulletmd.commands.register({
    id: "pdf-export.toggle-page-mode",
    title: "PDF: Toggle A4 Page Mode",
    keybinding: "Mod+Shift+P",
    run: togglePageMode,
  });
  bulletmd.menus.addItem({ label: "Toggle A4 Page Mode", run: togglePageMode });
  bulletmd.commands.register({ id: "pdf-export.print", title: "PDF: Print / Save as PDF…", run: printPdf });
  bulletmd.menus.addItem({ label: "Print / Save as PDF…", run: printPdf });
  bulletmd.statusBar.addItem({ id: "pdf-page-mode", text: "PDF: A4 mode", title: "Toggle A4 page boundaries", onClick: togglePageMode });
}
