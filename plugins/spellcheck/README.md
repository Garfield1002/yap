# Spell Check plugin

Spell Check underlines misspelled prose in the background and offers corrections
from the command palette or a right-click menu. It skips code, math, and URL
targets so markdown syntax and embedded code are not treated as prose.

## Install

Install this `spellcheck/` directory through **Settings → Plugins → Install
Plugin…**, then enable **Spell Check** from the Plugins settings. The status bar
shows the active dictionary, for example `spell: en_US`.

The plugin uses the system Hunspell dictionaries. Install at least one
dictionary for your distribution (typically an `hunspell-<language>` package).
If none is available, the status bar reports `spell: no dictionary` and the
plugin stays inactive.

## Use

- Right-click an underlined word to choose a replacement.
- Run **Spell Check: Suggest at Cursor** from the command palette, or press
  `Ctrl/Cmd+.` while the cursor is on an underlined word.

## Configure the language

After installing, create or edit the plugin's `data.json` file and set the
dictionary name exposed by your system:

```json
{ "lang": "en_US" }
```

Disable and re-enable the plugin after changing the setting.
