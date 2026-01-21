#!/bin/bash

set -euo pipefail

# Configuration
TARGET_FILE="src/views/login_demo.rs"

# Check that target file exists
if [ ! -f "$TARGET_FILE" ]; then
    echo "Error: target file for source modifications not found: $TARGET_FILE"
    exit 1
fi

# Function to make a small change to the code
make_change() {
    local counter=$1
    # Add a comment with a counter to trigger recompilation
    sed -i "1i\\
// Hot compile benchmark run #$counter" "$TARGET_FILE"
}

# Function to remove added comments
prepare_next() {
    # Remove comment lines added by benchmark
    sed -i '/^\/\/ Hot compile benchmark run #/d' "$TARGET_FILE"
}

# Export functions so they're available in subshells
export -f make_change
export -f prepare_next
export TARGET_FILE

# Run hyperfine benchmark
hyperfine \
    --warmup 2 \
    --max-runs 10 \
    --prepare "prepare_next" \
    --setup "make_change \$RANDOM" \
    "cargo +nightly build"
