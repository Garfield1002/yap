# Bundled dictionaries

`en_US.aff` / `en_US.dic` are the Hunspell `en_US` spelling dictionary, vendored
from the [spellbook](https://github.com/helix-editor/spellbook) project's
`vendor/en_US/` directory (which in turn takes them from the widely-distributed
SCOWL-derived `en_US` Hunspell dictionary).

They are embedded at compile time by `src/spellcheck.rs` (via `include_str!`)
and are only compiled in when the `spellcheck` feature is enabled.

These files are distributed under their original SCOWL / Hunspell license terms
(a permissive BSD-style license). See the upstream projects for the full text.
