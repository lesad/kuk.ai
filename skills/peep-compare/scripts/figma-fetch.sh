#!/usr/bin/env bash
# figma-fetch.sh — Fetch a Figma node as a rasterised image via REST /v1/images.
#
# Usage:
#   figma-fetch.sh <fileKey> <nodeId> [--scale N] [--format png|jpg|svg|pdf]
#                  [--absolute|--no-absolute] [--out PATH|-]
#
# Requires:
#   FIGMA_TOKEN  Personal access token (scope: file_content:read).
#   curl, jq
#
# Output:
#   Default path: ${TMPDIR:-/tmp}/figma-<fileKey>-<nodeId>-<scale>x.<format>
#                 (nodeId colons replaced with underscores for filename safety)
#   Prints that path on stdout for shell capture: DESIGN=$(figma-fetch.sh ...)
#
#   --out PATH    Write to PATH; print PATH on stdout.
#   --out -       Stream raw image bytes to stdout (nothing else printed there).
#
# Exit codes:
#   0   ok
#   1   HTTP / auth / network error
#   2   Figma reported err, or no image URL for the requested node
#   3   FIGMA_TOKEN not set
#   64  usage error

set -euo pipefail

die() { printf '%s\n' "$1" >&2; exit "${2:-1}"; }
usage() {
  sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 64
}

[[ -n "${FIGMA_TOKEN:-}" ]] || die "FIGMA_TOKEN not set" 3

scale=2
format=png
absolute=true
out=""
file_key=""
node_id=""

while (( $# )); do
  case "$1" in
    --scale)       scale="${2:?missing value for --scale}"; shift 2 ;;
    --format)      format="${2:?missing value for --format}"; shift 2 ;;
    --absolute)    absolute=true;  shift ;;
    --no-absolute) absolute=false; shift ;;
    --out)         out="${2:?missing value for --out}"; shift 2 ;;
    -h|--help)     usage ;;
    --)            shift; break ;;
    -*)            die "unknown flag: $1" 64 ;;
    *)
      if   [[ -z "$file_key" ]]; then file_key="$1"
      elif [[ -z "$node_id"  ]]; then node_id="$1"
      else die "extra positional arg: $1" 64
      fi
      shift
      ;;
  esac
done

[[ -n "$file_key" && -n "$node_id" ]] || usage

# API needs colon form; filesystem needs colon-free form.
api_node_id="${node_id//-/:}"
fs_node_id="${api_node_id//:/_}"

if [[ -z "$out" ]]; then
  out="${TMPDIR:-/tmp}/figma-${file_key}-${fs_node_id}-${scale}x.${format}"
fi

api_url="https://api.figma.com/v1/images/${file_key}?ids=${api_node_id}&format=${format}&scale=${scale}"
[[ "$absolute" == true ]] && api_url="${api_url}&use_absolute_bounds=true"

response=$(curl -fsS -H "X-Figma-Token: ${FIGMA_TOKEN}" "$api_url") \
  || die "Figma API request failed (HTTP / network / auth)" 1

err=$(printf '%s' "$response" | jq -r '.err // ""')
[[ -z "$err" ]] || die "Figma error: $err" 2

image_url=$(printf '%s' "$response" | jq -r --arg id "$api_node_id" '.images[$id] // ""')
[[ -n "$image_url" && "$image_url" != "null" ]] \
  || die "no image URL returned for node $api_node_id" 2

if [[ "$out" == "-" ]]; then
  curl -fsSL "$image_url"
else
  curl -fsSL "$image_url" -o "$out"
  printf '%s\n' "$out"
fi
