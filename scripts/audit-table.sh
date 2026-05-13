#!/usr/bin/env bash
# Dependency audit table formatter. Requires: cargo-audit.
set -euo pipefail

RED=$(tput setaf 1 2>/dev/null || echo '')
YELLOW=$(tput setaf 3 2>/dev/null || echo '')
GREEN=$(tput setaf 2 2>/dev/null || echo '')
BOLD=$(tput bold 2>/dev/null || echo '')
RST=$(tput sgr0 2>/dev/null || echo '')
DIM="\033[2m"
DKYLW="\033[33m"

TMPFILE=$(mktemp /tmp/audit-table.XXXXXX)
trap 'rm -f "$TMPFILE" "${TMPFILE}.parsed" "${TMPFILE}.full"' EXIT

HAS_EXCEPTIONS=false
[ -f .cargo/audit.toml ] && HAS_EXCEPTIONS=true

# ── Reason map for suppressed advisories ─────
# Format: RUSTSEC-ID=reason
declare -A REASONS=(
    ["RUSTSEC-2023-0071"]="no patched version exists"
    ["RUSTSEC-2025-0040"]="transitive via pam, no fix"
    ["RUSTSEC-2023-0040"]="transitive via pam"
    ["RUSTSEC-2023-0059"]="transitive via pam"
    ["RUSTSEC-2025-0134"]="TODO: no drop-in replacement"
    ["RUSTSEC-2025-0069"]="TODO: non-trivial migration"
    ["RUSTSEC-2021-0137"]="TODO: heavily used, non-trivial"
    ["RUSTSEC-2025-0057"]="transitive via sled"
    ["RUSTSEC-2024-0384"]="transitive via sled"
    ["RUSTSEC-2024-0436"]="transitive via rustpython"
    ["RUSTSEC-2025-0075"]="transitive via rustpython"
    ["RUSTSEC-2025-0080"]="transitive via rustpython"
    ["RUSTSEC-2025-0081"]="transitive via rustpython"
    ["RUSTSEC-2025-0098"]="transitive via rustpython"
    ["RUSTSEC-2025-0102"]="transitive via rustpython"
    ["RUSTSEC-2021-0141"]="migrated to dotenvy"
    ["RUSTSEC-2025-0141"]="transitive, cannot remove"
)

# ── Run with current exceptions ──────────────
cargo audit 2>&1 | tee "$TMPFILE" >/dev/null || true

# ── Parse helper (extracts id field too) ─────
parse_audit() {
    local inf="$1" out="$2"
    awk '
    BEGIN { RS=""; FS="\n" }
    {
        crate=""; version=""; title=""; severity=""; fix="-"; warn_type=""; id=""
        for (i=1; i<=NF; i++) {
            line = $i
            if (line ~ /^Crate:/)      { split(line, a, /[[:space:]]+/); crate=a[2] }
            if (line ~ /^Version:/)    { split(line, a, /[[:space:]]+/); version=a[2] }
            if (line ~ /^Title:/)      { sub(/^Title:[[:space:]]+/, "", line); title=line }
            if (line ~ /^ID:/)         { split(line, a, /[[:space:]]+/); id=a[2] }
            if (line ~ /^Severity:/)   { split(line, a, /[[:space:]]+/); score=a[2];
                                         sub(/^Severity:[[:space:]]+[^[:space:]]+[[:space:]]+/, "", line); sev=line;
                                         gsub(/[()]/, "", sev); severity=sprintf("%s (%s)", score, sev) }
            if (line ~ /^Warning:/)    { split(line, a, /[[:space:]]+/); warn_type=a[2] }
            if (line ~ /^Solution:/)   { sub(/^Solution:[[:space:]]+/, "", line); fix=line;
                                         if (fix ~ /No fixed/) fix="-"; gsub(/^Upgrade to /, "", fix) }
        }
        if (length(crate) == 0) next
        if (length(warn_type) > 0)
            printf "warn\t%s\t%s\t%s\t%s\t%s\t%s\n", warn_type, crate, version, title, fix, id
        else
            printf "vuln\t%s\t%s\t%s\t%s\t%s\t%s\n", (length(severity)>0 ? severity : "?"), crate, version, title, fix, id
    }
    ' "$inf" > "$out"
}

parse_audit "$TMPFILE" "${TMPFILE}.parsed"

# ── If exceptions exist, also get the full picture ──
if $HAS_EXCEPTIONS; then
    FULL_TMP=$(mktemp /tmp/audit-full.XXXXXX)
    mv .cargo/audit.toml .cargo/_audit.toml.bak
    cargo audit 2>&1 | tee "$FULL_TMP" >/dev/null || true
    mv .cargo/_audit.toml.bak .cargo/audit.toml
    parse_audit "$FULL_TMP" "${TMPFILE}.full"
    rm -f "$FULL_TMP"

    comm -23 <(sort "${TMPFILE}.full") <(sort "${TMPFILE}.parsed") > "${TMPFILE}.ignored"
fi

# ── Lookup reason by advisory id ─────────────
reason_for() {
    local id="$1" r
    r="${REASONS[$id]:-}"
    [ -n "$r" ] && echo "$r" || echo "see .cargo/audit.toml"
}

# ── Header ───────────────────────────────────
echo -e "${BOLD}  Dependency Audit Report${RST}"
echo

# ── Vulnerabilities (still open) ─────────────
VULN_COUNT=$(awk '/^vuln/{n++} END{print n+0}' "${TMPFILE}.parsed")
if [ "$VULN_COUNT" -gt 0 ]; then
    echo -e "  ${BOLD}${RED}VULNERABILITIES${RST} ${RED}(${VULN_COUNT} found)${RST}"
    echo
    printf "  %-20s %-10s %-14s %-55s %-30s\n" "Crate" "Version" "Severity" "Issue" "Fix"
    printf "  %-20s %-10s %-14s %-55s %-30s\n" "--------------------" "----------" "--------------" "-------------------------------------------------------" "------------------------------"
    grep '^vuln' "${TMPFILE}.parsed" | while IFS=$'\t' read -r _ sev crate ver title fix id; do
        printf "  ${RED}%-20s${RST} %-10s %-14s %-55s %-30s\n" \
            "${crate:0:20}" "${ver:0:10}" "${sev:0:14}" "${title:0:55}" "${fix:0:30}"
    done
    echo
fi

# ── Warnings (still open) ────────────────────
WARN_COUNT=$(awk '/^warn/{n++} END{print n+0}' "${TMPFILE}.parsed")
if [ "$WARN_COUNT" -gt 0 ]; then
    echo -e "  ${BOLD}${YELLOW}WARNINGS${RST} ${YELLOW}(${WARN_COUNT} total)${RST}"
    echo
    printf "  %-14s %-20s %-10s %-60s\n" "Kind" "Crate" "Version" "Issue"
    printf "  %-14s %-20s %-10s %-60s\n" "--------------" "--------------------" "----------" "------------------------------------------------------------"
    grep '^warn' "${TMPFILE}.parsed" | while IFS=$'\t' read -r _ kind crate ver title fix id; do
        printf "  ${YELLOW}%-14s${RST} %-20s %-10s %-60s\n" \
            "${kind:0:14}" "${crate:0:20}" "${ver:0:10}" "${title:0:60}"
    done
    echo
fi

# ── Clean summary ────────────────────────────
if [ "$VULN_COUNT" -eq 0 ] && [ "$WARN_COUNT" -eq 0 ]; then
    echo -e "  ${BOLD}${GREEN}All clear — no open findings${RST}"
    echo
fi

# ── Ignored advisories summary ───────────────
if $HAS_EXCEPTIONS; then
    IGN_VULN=$(awk '/^vuln/{n++} END{print n+0}' "${TMPFILE}.ignored" 2>/dev/null)
    IGN_WARN=$(awk '/^warn/{n++} END{print n+0}' "${TMPFILE}.ignored" 2>/dev/null)
    IGN_TOTAL=$((IGN_VULN + IGN_WARN))

    if [ "$IGN_TOTAL" -gt 0 ]; then
        echo -e "  ${BOLD}${DIM}Suppressed by .cargo/audit.toml${RST} ${DIM}(${IGN_TOTAL} advisories)${RST}"
        echo
        printf "  ${DIM}%-20s %-8s %-7s %-55s %-28s${RST}\n" "Crate" "Version" "Risk" "Issue" "Why suppressed"
        printf "  ${DIM}%-20s %-8s %-7s %-55s %-28s${RST}\n" "--------------------" "--------" "-------" "-------------------------------------------------------" "----------------------------"

        # vulns first
        grep '^vuln' "${TMPFILE}.ignored" 2>/dev/null | while IFS=$'\t' read -r _ sev crate ver title fix id; do
            reason=$(reason_for "$id")
            if [[ "$reason" == TODO:* ]]; then color="${DKYLW}"; else color="${DIM}"; fi
            printf "  ${color}%-20s %-8s %-7s %-55s %-28s${RST}\n" \
                "${crate:0:20}" "${ver:0:8}" "${sev:0:7}" "${title:0:55}" "$reason"
        done
        # then warnings
        grep '^warn' "${TMPFILE}.ignored" 2>/dev/null | while IFS=$'\t' read -r _ kind crate ver title fix id; do
            reason=$(reason_for "$id")
            if [[ "$reason" == TODO:* ]]; then color="${DKYLW}"; else color="${DIM}"; fi
            printf "  ${color}%-20s %-8s %-7s %-55s %-28s${RST}\n" \
                "${crate:0:20}" "${ver:0:8}" "${kind:0:7}" "${title:0:55}" "$reason"
        done
        echo
    fi
fi
