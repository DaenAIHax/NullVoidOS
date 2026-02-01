#!/bin/bash
# Script to initialize a Headless Kali Linux container on NullVoidOS

echo "🛠️  Preparing your hacking environment..."

# 1. Create the container (using -Y for non-interactive setup)
distrobox create --image docker.io/kalilinux/kali-rolling --name kali -Y

# 2. Install Kali Headless metapackage and essential tools
echo "🚀 Installing Kali Headless (Metapackages) and Core Tools..."

# It is more efficient to run these in a single 'enter' session
distrobox enter kali -- bash -c "
    sudo apt-get update && \
    sudo apt-get install -y kali-linux-headless && \
    sudo apt-get clean
"

echo "✅ Done! To enter your Kali environment, type: distrobox enter kali"
