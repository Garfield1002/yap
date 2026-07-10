// A minimal bulletmd plugin. Copy this directory to `$BULLETMD_HOME/plugins/hello/`, then
// enable it under Settings → Plugins. It touches every simple surface: a status
// bar item, a command (in the palette, keybound, and in the Plugins menu), and a
// live-preview builder that dims literal "TODO" markers in the text.
//
// Note there are no `@codemirror/*` imports: the app's module instances arrive
// on `bulletmd.cm`, so a plugin never carries (and never duplicates) CodeMirror.

export function activate(bulletmd) {
  bulletmd.statusBar.addItem({ id: "hello", text: "👋 hello", title: "Hello plugin is active" });

  bulletmd.commands.register({
    id: "hello.greet",
    title: "Hello: Greet",
    keybinding: "Mod+Shift+H",
    run: () => alert("Hello from a bulletmd plugin!"),
  });

  bulletmd.menus.addItem({ label: "Say hello", run: () => alert("hi") });

  // Dim every literal "TODO" that appears in an inline-code span. Keys off the
  // Lezer node name, exactly like the built-in builders.
  bulletmd.livePreview.registerBuilder(["InlineCode"], (node, b) => {
    const text = b.state.doc.sliceString(node.from, node.to);
    if (text.includes("TODO")) b.mark(node.from, node.to, "hello-todo");
  });
}
