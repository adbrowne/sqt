#!/bin/bash
# Launch Claude Code inside the Docker container
# This script ensures the container is running and then executes Claude Code CLI

set -e

echo "🐳 Starting smelt development container..."
docker-compose up -d

echo "🤖 Launching Claude Code inside container..."
echo ""
docker-compose exec smelt-dev claude

echo ""
echo "👋 Claude Code session ended."
