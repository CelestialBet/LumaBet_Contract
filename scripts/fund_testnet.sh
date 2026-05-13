#!/usr/bin/env bash
# fund_testnet.sh — Fund one or more Stellar testnet accounts via Friendbot
set -euo pipefail

FRIENDBOT_URL="https://friendbot.stellar.org"

usage() {
  echo "Usage: $0 <PUBLIC_KEY> [<PUBLIC_KEY2> ...]"
  echo "  Funds each address with 10,000 test XLM via Stellar Friendbot."
  exit 1
}

[[ $# -lt 1 ]] && usage

fund_account() {
  local pubkey="$1"
  echo "[fund] Funding $pubkey..."
  local response
  response=$(curl -s -w "\n%{http_code}" "${FRIENDBOT_URL}?addr=${pubkey}")
  local body http_code
  body=$(echo "$response" | head -n -1)
  http_code=$(echo "$response" | tail -n 1)

  if [[ "$http_code" == "200" ]]; then
    echo "[fund] Success: $pubkey funded."
  elif echo "$body" | grep -q "createAccountAlreadyExist"; then
    echo "[fund] $pubkey already funded — skipping."
  else
    echo "[error] Failed to fund $pubkey (HTTP $http_code): $body" >&2
    return 1
  fi
}

for key in "$@"; do
  fund_account "$key"
done

echo "[fund] All done."
