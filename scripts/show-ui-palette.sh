#!/usr/bin/env bash

set -euo pipefail

show_color() {
  local name="$1"
  local index="$2"

  printf '\033[38;5;%sm█████████\033[0m  %s   Color::Indexed(%s)\n' "$index" "$name" "$index"
}

show_group() {
  local title="$1"
  shift
  printf '%s\n' "$title"
  while (($#)); do
    show_color "$1" "$2"
    shift 2
  done
}

show_group '-- Dialogs/placeholders' \
  'BG_0' 232 \
  'BG_1' 233 \
  'BG_2' 235 \
  'BG_3' 237 \
  'SURFACE' 236 \
  'GRAY_0' 239 \
  'GRAY_1' 244 \
  'GRAY_2' 249 \
  'FG' 253 \
  'MUTED' 243 \
  'FAINT' 238 \
  'BORDER' 240 \
  'ON_HIGHLIGHT' 235

show_group '-- Errors' \
  'ERROR_BASE' 52 \
  'ERROR_GLOW' 88 \
  'ERROR_HEAT' 125 \
  'ERROR_PEAK' 161 \
  'ERROR' 197

show_group '-- Warnings' \
  'WARNING_BASE' 58 \
  'WARNING_GLOW' 94 \
  'WARNING_HEAT' 130 \
  'WARNING_PEAK' 166 \
  'WARNING' 172

show_group '-- Success' \
  'SUCCESS_BASE' 23 \
  'SUCCESS_GLOW' 29 \
  'SUCCESS_HEAT' 35 \
  'SUCCESS_PEAK' 42 \
  'SUCCESS' 41 \
  'ACCENT' 36

show_group '-- Processing' \
  'PROCESSING_BASE' 54 \
  'PROCESSING_GLOW' 91 \
  'PROCESSING_HEAT' 127 \
  'PROCESSING_PEAK' 163 \
  'PROCESSING' 169 \
  'PRIMARY' 200 \
  'SECONDARY' 98 \
  'HIGHLIGHT' 134
