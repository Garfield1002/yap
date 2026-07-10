# Page Break plugin

Page Break adds deterministic A4 page spacing to the editor. It is designed to
work with the [PDF Export](../pdf-export/README.md) plugin, which owns the A4
page-mode switch and print command.

## Install

Install and enable **PDF Export** first. Then install this `page-break/`
directory through **Settings → Plugins → Install Plugin…** and enable **Page
Break**. The plugin declares PDF Export as a requirement, so it will not load
without it.

## Use

With A4 page mode enabled, Page Break keeps whole lines out of the bottom
margin and inserts the appropriate visual gap before the next page. To force a
break at a precise point, put this marker on its own line:

```markdown
<!--page-break-->
```

The marker becomes a page boundary in both A4 page mode and PDF printing.
