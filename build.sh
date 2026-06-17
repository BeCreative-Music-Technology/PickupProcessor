#!/bin/bash

# Pull latest changes from main
git checkout main
git pull
echo "Fetched latest changes from repository"

# Clean project and build as release
cargo clean
cargo build --release
echo "Built project"
