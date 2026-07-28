#!/bin/bash
# Creates a new dbmate migration inside migration/<module>.
# Usage: ./scripts/migrate/dbmate.sh iam create_something_table
set -euo pipefail

module="$1"
name="$2"

mkdir -p "$(pwd)/migration/$module"

docker run --rm -it --network=host \
  -v "$(pwd)/migration/$module:/db/migrations" \
  --user "$(id -u):$(id -g)" \
  ghcr.io/amacneil/dbmate new "$name"
