# BulletMD retained GPUI POC

This Fedora-only prototype tests BulletMD's live Markdown interaction directly
on published `gpui` 0.2.2. It is isolated from the production Tauri app.

## Retained architecture

The document is parsed once at open into stable block objects. Each block owns
its source range, source lines, parsed/styled rendering, rendered-to-source byte
map, and cached raw/rendered GPUI shapes. Cached row counts define the complete
scroll extent.

Normal text input changes only the active block's raw source and invalidates
only its raw shape. Inactive parsed and shaped blocks retain their identity and
caches. Navigation commits the old block and reveals the new one. Enter locally
resegments only the active region. Backspace/Delete at a block boundary fuse
exactly the adjacent pair; the bounded parser cannot let an unmatched fence
consume a third block. A cross-block selection reveals its contiguous blocks
and replacement locally resegments that range. Escape is the deliberate global
commit: it reparses all source, assigns new block identities, and leaves editing.

The surface is exactly 768 px wide, with a 720 px shaped-text column. It does
not reshape when the window resizes: wide viewports center it and narrow ones
scroll horizontally. Baselines are exactly `48 + 24n`; cached block heights are
whole 24 px rows. Painting visits only visible blocks plus one viewport of
overscan, while grid dots are limited to the current viewport.

Rendered lines retain byte-boundary maps into source, so one click places the
caret in the revealed source and hidden punctuation snaps to the nearest mapped
boundary. Bold, italic, inline code, links, lists, headings, and fenced code use
resolved GPUI font/style runs.

## Build and run

Fedora requires:

```bash
sudo dnf install libxkbcommon-x11-devel
```

Then:

```bash
cargo test
cargo build --release --bin bulletmd-native-poc
./target/release/bulletmd-native-poc fixtures/sample.md
```

Controls include arrows, Shift+arrows, Ctrl+Left/Right word navigation,
Ctrl+Shift+Left/Right word selection, Ctrl+Backspace word deletion, Home/End,
clipboard commands, Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y, Ctrl+S, and Escape. Markdown
shortcuts exactly match the existing CodeMirror commands:

- Ctrl+B toggles `**`
- Ctrl+I toggles `*`
- Ctrl+E toggles backticks
- Ctrl+K replaces the selection with `[selection]()` and places the caret in
  the URL

The native File menu supports new documents and windows, open and recent files,
save/save-as, rename, delete, copy path, and reveal in the system file manager.
Saved files autosave after five quiet seconds through an atomic same-directory
replacement. A parent-directory watcher handles rename-based external saves:
clean buffers reload, while dirty buffers show a Keep Mine / Load Disk conflict
choice. Closing or switching away from dirty content requires confirmation.

Settings shares the production `state.json` and provides System/Light/Dark
themes plus dot-grid opacity presets from 0% to 30%.

Standalone Markdown images render in place while inactive and reveal their raw
`![alt](source)` when clicked. Filesystem paths resolve relative to the note,
absolute paths and HTTP(S) URLs use GPUI's asynchronous image cache, and image
height snaps to the same 24 px grid as text. Pasting an image stores it under
`BULLETMD_HOME/assets/` and inserts an absolute Markdown reference.

## Verification and instrumentation

Pure retained-model behavior and mappings are covered by eight tests. They
prove that 100 ordinary inserts parse zero blocks, preserve inactive block
identities, and invalidate only the active ID; Enter reparses one bounded
region; a boundary deletion fuses exactly two blocks; an unmatched local fence
does not absorb the third block; and ordered-list/source mappings survive.

The display-independent release benchmark is:

```bash
cargo build --release --bin model_bench
./target/release/model_bench fixtures/sample.md
```

On the development Fedora machine after this rewrite:

| operation | release time |
| --- | ---: |
| 100 ordinary retained-model inserts | 0.156-0.163 ms total |
| Enter local resegment | 0.008-0.012 ms |
| boundary fusion | 0.005-0.007 ms |
| Escape global reparse | 0.144-0.168 ms |

These are model costs, not paint latency. For real edit-to-paint measurement,
the app accepts `BULLETMD_BENCH_EDIT=100`; after first paint it inserts one
character per frame and emits `EDIT_LATENCY_100 median_ms=... p95_ms=...
max_ms=...`. Every ordinary edit also records receipt-to-next-CPU-paint latency.

```bash
BULLETMD_BENCH_EDIT=100 ./target/release/bulletmd-native-poc fixtures/sample.md
./benchmark.sh native
```

The rewritten release app was launched against the real Fedora Wayland
compositor. Two warmed retained-renderer runs measured 65.84-65.96 MiB PSS,
71.57-71.68 MiB RSS, and 106.958-116.577 ms to first CPU-side paint. Additional
post-link/cooler runs ranged up to 314.024 ms, so the strict cold 250 ms startup
target is not consistently met.

Two 100-edit animation-frame runs measured:

| run | median | p95 | maximum |
| --- | ---: | ---: | ---: |
| warmed 1 | 16.697 ms | 17.570 ms | 43.857 ms |
| warmed 2 | 16.640 ms | 17.518 ms | 18.449 ms |

The counters were exactly `raw_reshapes=101` (one initial active shape plus 100
edits), `rendered_reshapes=71` (initial blocks only), `local_reparses=0`, and
`parsed_blocks=0`. This proves the retained invalidation path, but the measured
distribution **fails** the median <4 ms, p95 <8 ms, and maximum <16.7 ms targets.
The synthetic driver deliberately receives each edit just after a paint and
requests the following animation frame, so its approximately 16.7 ms floor
includes one 60 Hz frame interval. Real keyboard-event sampling is still needed
to separate scheduling delay from CPU parse/shape/paint work; the model work for
100 inserts is only 0.156-0.163 ms total.

## Known limitations

- The bounded block scanner is deliberately smaller than CommonMark.
- Undo/redo applies compact edit transactions but currently performs a global
  cache rebuild when traversing history.
- IME uses GPUI's platform handler and local UTF-16/UTF-8 conversion, but Fedora
  compose/dead-key behavior still needs a real-desktop smoke test.
- Caret auto-scroll and two-dimensional wheel/trackpad behavior use
  `ScrollHandle`; the automated compositor launch exercised painting and frame
  scheduling, but manual wheel/trackpad interaction remains to be checked.
- Accessibility completion, bidi validation, plugins, inline mixed-text images,
  animated-image frame advancement, math, tables,
  export, packaging, macOS, and Windows remain out of scope.

## Recommendation

The retained design removes the known whole-document parse/shape/paint path and
its invariants are test-backed. It is ready for desktop latency and scroll
validation, not for a production migration decision. Continue only if the
instrumented Fedora run meets the edit-latency targets without regressing the
previous memory advantage.
