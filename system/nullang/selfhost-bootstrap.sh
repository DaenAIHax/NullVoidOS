#!/usr/bin/env bash
# selfhost-bootstrap.sh — certifica il self-host di Nullang per punto fisso.
#
# SPEC §12. Il compilatore-in-Nullang vive in examples/selfhost-parser.null
# (lexer + parser + codegen, tutto in Nullang). Questo script ne dimostra il
# bootstrap a stadi, l'analogo del `bootstrap-tools` di Nix:
#
#   stage0  = il compilatore Rust (`nullang`, il SEME). Reperito via nix o
#             dalla variabile $NULLANG. È l'unica radice di fiducia esterna.
#   stage1  = il compilatore-in-Nullang, compilato DAL seme:
#             stage0 esegue il sorgente → emette il C del compilatore → gcc.
#   stage2  = lo stesso, ricompilato DA stage1 (non più da Rust).
#
# Quando run, il binario legge il proprio sorgente (argv1) ed emette il C del
# compilatore nel file dato (argv2). GATE del punto fisso:
#   - il C emesso da stage0, stage1, stage2 è BYTE-IDENTICO  (self-host stabile);
#   - lo stdout dei tre stadi è identico                      (comportamento).
# Se entrambi reggono, Rust è solo il seme e può essere tolto.
#
# Uso (dalla root del repo, dentro `nix develop`):
#   nix develop --command bash system/nullang/selfhost-bootstrap.sh
# Variabili: NULLANG=<path al nullang Rust> (default: result/bin/nullang),
#            CC=<compilatore C> (default: cc; nel devShell usare gcc).

set -euo pipefail

cd "$(dirname "$0")/../.."        # → root del repo  (cwd richiesta: src_path è relativo)
ROOT=$(pwd)
SRC=system/nullang/examples/selfhost-parser.null
NULLANG=${NULLANG:-$ROOT/result/bin/nullang}
CC=${CC:-cc}
W=$(mktemp -d)
trap 'rm -rf "$W"' EXIT

say() { printf '\033[1m%s\033[0m\n' "$*"; }

[ -x "$NULLANG" ] || { echo "seme Rust non trovato: $NULLANG (prova: nix build .#nullang)"; exit 2; }
command -v "$CC" >/dev/null || { echo "compilatore C non trovato: $CC (sei in nix develop?)"; exit 2; }

say "stage0 — il seme Rust compila il compilatore-in-Nullang"
# `build` emette+compila col compilatore Rust; stampa il path dell'ELF.
STAGE0=$(CC="$CC" "$NULLANG" build "$SRC" | tail -1)
"$STAGE0" "$SRC" "$W/self0.c" > "$W/out0.txt"
"$CC" -O2 -o "$W/stage1" "$W/self0.c"

say "stage1 — il compilatore-in-Nullang (compilato dal seme) ricompila sé stesso"
"$W/stage1" "$SRC" "$W/self1.c" > "$W/out1.txt"
"$CC" -O2 -o "$W/stage2" "$W/self1.c"

say "stage2 — ricompilato da stage1 (Rust fuori dal giro)"
"$W/stage2" "$SRC" "$W/self2.c" > "$W/out2.txt"

say "gate 1/2 — il C emesso è byte-identico fra gli stadi"
if diff -q "$W/self0.c" "$W/self1.c" >/dev/null && diff -q "$W/self1.c" "$W/self2.c" >/dev/null; then
  echo "  OK: self0.c == self1.c == self2.c ($(wc -c < "$W/self1.c") byte)"
else
  echo "  FALLITO: il C diverge fra gli stadi"; exit 1
fi

say "gate 2/2 — il comportamento (stdout) è identico fra gli stadi"
# Si esclude la sola riga "C di sé stesso: … → <path>": il path di uscita è
# diverso per stadio per costruzione (self0/1/2.c), non è una divergenza.
for s in 0 1 2; do grep -v 'C di sé stesso:' "$W/out$s.txt" > "$W/b$s.txt"; done
if diff -q "$W/b0.txt" "$W/b1.txt" >/dev/null && diff -q "$W/b1.txt" "$W/b2.txt" >/dev/null; then
  echo "  OK: out0 == out1 == out2 (a meno del path di uscita)"
else
  echo "  FALLITO: il comportamento diverge fra gli stadi"; exit 1
fi

say "PUNTO FISSO RAGGIUNTO — il self-host è certificato. Il seme Rust è rimovibile."
