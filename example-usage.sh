#!/bin/bash

# Example usage script for debugging CircleCI build failures

echo "CircleCI Debug Tool - Example Usage"
echo "===================================="
echo ""
echo "First, set your CircleCI token:"
echo "export CIRCLECI_TOKEN='your-token-here'"
echo ""
echo "Then use the tool with your failed build URLs:"
echo ""

# Example from your PR failures
echo "# Debug release job failure:"
echo "circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156093"
echo ""

echo "# Debug E2E Browserstack test failure:"
echo "circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156100"
echo ""

echo "# Debug test job failure:"
echo "circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156095"
echo ""

echo "# To also fetch and display logs:"
echo "CIRCLE_DEBUG_FETCH_LOGS=1 circle-debug build https://circleci.com/gh/stitchfix/web-frontend/156093"
echo ""

echo "# After building:"
echo "cargo build --release"
echo "# The binary will be at: ./target/release/circle-debug"
echo ""
echo "# Install globally:"
echo "cargo install --path ."