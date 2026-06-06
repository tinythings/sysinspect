#!/usr/bin/env bash

set -euo pipefail

ansi_block() {
  local index="$1"
  printf '\033[38;5;%sm█████████\033[0m' "$index"
}

indexed_rgb() {
  local index="$1"

  if (( index < 0 || index > 255 )); then
    return 1
  fi

  case "$index" in
    0) echo "00 00 00" ;;
    1) echo "80 00 00" ;;
    2) echo "00 80 00" ;;
    3) echo "80 80 00" ;;
    4) echo "00 00 80" ;;
    5) echo "80 00 80" ;;
    6) echo "00 80 80" ;;
    7) echo "c0 c0 c0" ;;
    8) echo "80 80 80" ;;
    9) echo "ff 00 00" ;;
    10) echo "00 ff 00" ;;
    11) echo "ff ff 00" ;;
    12) echo "00 00 ff" ;;
    13) echo "ff 00 ff" ;;
    14) echo "00 ff ff" ;;
    15) echo "ff ff ff" ;;
    *)
      if (( index >= 16 && index <= 231 )); then
        local cube=$((index - 16))
        local r_index=$((cube / 36))
        local g_index=$(((cube / 6) % 6))
        local b_index=$((cube % 6))
        local steps=(0 95 135 175 215 255)

        printf '%02x %02x %02x\n' \
          "${steps[r_index]}" \
          "${steps[g_index]}" \
          "${steps[b_index]}"
      else
        local gray=$((8 + (index - 232) * 10))
        printf '%02x %02x %02x\n' "$gray" "$gray" "$gray"
      fi
      ;;
  esac
}

for index in $(seq 0 255); do
  read -r r g b < <(indexed_rgb "$index")
  printf '%s  Color::Rgb(0x%s, 0x%s, 0x%s)   Color::Indexed(%3d)\n' \
    "$(ansi_block "$index")" \
    "${r^^}" \
    "${g^^}" \
    "${b^^}" \
    "$index"
done
