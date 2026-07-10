# PDF Export plugin

PDF Export adds an A4 page-mode preview to the editor and sends the current
markdown document to the system print dialog, where it can be saved as a PDF.

## Install

Install this `pdf-export/` directory through **Settings → Plugins → Install
Plugin…**, then enable **PDF Export** from the Plugins settings.

## Use

- Run **PDF: Toggle A4 Page Mode** from the command palette or press
  `Ctrl/Cmd+Shift+P` to show or hide A4 page boundaries in the editor.
- Run **PDF: Print / Save as PDF…** to open the system print dialog.

Page mode is remembered in the plugin's `data.json` after it has been toggled.
For deliberate page boundaries, also install and enable the
[Page Break](../page-break/README.md) plugin.
