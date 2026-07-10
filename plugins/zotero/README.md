# Zotero Citations

Searches the Zotero Desktop Local API and inserts a Pandoc-style citation such
as `[@doe2024]` at the cursor.

Enable Zotero's **Settings → Advanced → Allow other applications on this
computer to communicate with Zotero** setting, then install this folder from
yap's **Settings → Plugins** menu. The Local API is read-only and stays on the
loopback interface.

Use **Zotero: Insert Citation…** from the command palette, the Plugins menu,
or the status bar. Search by title or creator, choose a result, and the
citation is inserted at the cursor.

Outside the active cursor range, a citation renders as a small numbered
reference such as `[1]`. Add a line containing `<!--bibliography-->` to render
the bibliography in first-citation order:

```markdown
Research supports this claim [@doe2024].

<!--bibliography-->
```

The resulting entry is `[1] title, year, authors`. Metadata is remembered when
you insert a citation through the picker. Existing citations are resolved from
Zotero in the background; a metadata-unavailable placeholder remains only when
the matching library item cannot be found.

The plugin uses a `Citation Key:` line in the item's Zotero **Extra** field
(as written by Better BibTeX) when available. Otherwise it uses Zotero's
eight-character item key. Configure a non-default library by creating
`data.json` in the installed plugin directory:

```json
{
  "library": "users/0"
}
```

For a group library, use `"library": "groups/<group-id>"`.
