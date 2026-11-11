#!/usr/bin/env bash
# Télécharge le binaire pdfium ÉPINGLÉ (bblanchon/pdfium-binaries) pour Linux x64 et vérifie son sha256.
#   scripts/pdfium.sh [dossier]      (défaut : ~/.cache/fragment-rs/pdfium)
# Écrit le dossier de la bibliothèque sur la sortie standard, et PDFIUM_LIB_DIR dans $GITHUB_ENV en CI.
# ★ Mêmes version et empreintes que scripts/pdfium.ps1 : les changer ensemble.
set -euo pipefail

VERSION='chromium/8125'
SHA256='e1c3ce6e270cb5e0c20816b379ad9f2c1309972697c505f508de92bf5e7d6659' # pdfium-linux-x64.tgz
URL="https://github.com/bblanchon/pdfium-binaries/releases/download/${VERSION}/pdfium-linux-x64.tgz"

dest="${1:-$HOME/.cache/fragment-rs/pdfium}"
cible="$dest/${VERSION//\//-}"
if [ ! -f "$cible/lib/libpdfium.so" ]; then
  mkdir -p "$cible"
  curl -fsSL "$URL" -o "$cible/pdfium.tgz"
  echo "$SHA256  $cible/pdfium.tgz" | sha256sum -c - >&2
  tar -xzf "$cible/pdfium.tgz" -C "$cible"
  rm "$cible/pdfium.tgz"
fi
lib="$cible/lib"
if [ -n "${GITHUB_ENV:-}" ]; then echo "PDFIUM_LIB_DIR=$lib" >> "$GITHUB_ENV"; fi
echo "$lib"
