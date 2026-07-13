#!/usr/bin/env bash
set -euo pipefail

mode="${1:-native}"
root="$(cd "$(dirname "$0")/.." && pwd)"
poc="$root/native-poc"
fixture="$poc/fixtures/sample.md"
log="${TMPDIR:-/tmp}/bulletmd-${mode}-benchmark.log"

case "$mode" in
  native) command=("$poc/target/release/bulletmd-native-poc" "$fixture") ;;
  tauri) command=("${TAURI_BIN:-$root/src-tauri/target/release/bulletmd}" "$fixture") ;;
  *) echo "usage: $0 native|tauri" >&2; exit 2 ;;
esac

if [[ ! -x "${command[0]}" ]]; then
  echo "missing release binary: ${command[0]}" >&2
  exit 2
fi

: >"$log"
"${command[@]}" 2>"$log" &
root_pid=$!
trap 'kill "$root_pid" 2>/dev/null || true' EXIT INT TERM
sleep 5

pids=("$root_pid")
for ((i=0; i<${#pids[@]}; i++)); do
  while read -r child; do
    [[ -n "$child" ]] && pids+=("$child")
  done < <(pgrep -P "${pids[$i]}" || true)
done

pss_kib=0
rss_kib=0
for pid in "${pids[@]}"; do
  [[ -r "/proc/$pid/smaps_rollup" ]] || continue
  pss=$(awk '/^Pss:/ { print $2 }' "/proc/$pid/smaps_rollup")
  rss=$(awk '/^Rss:/ { print $2 }' "/proc/$pid/smaps_rollup")
  pss_kib=$((pss_kib + ${pss:-0}))
  rss_kib=$((rss_kib + ${rss:-0}))
done

echo "mode=$mode"
echo "root_pid=$root_pid process_count=${#pids[@]}"
awk -v kib="$pss_kib" 'BEGIN { printf "pss_mib=%.2f\n", kib / 1024 }'
awk -v kib="$rss_kib" 'BEGIN { printf "rss_mib=%.2f\n", kib / 1024 }'
if [[ -s "$log" ]]; then
  grep -E 'FIRST_INTERACTIVE_FRAME_MS=|EDIT_LATENCY_100|CACHE_COUNTERS' "$log" || true
fi
