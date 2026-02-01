#!/bin/bash
# Script to initialize a ROOT Kali Linux container on NullVoidOS

echo "🛠️  Preparing your ROOT hacking environment..."

# 1. Create the container with ROOT privileges
# Abbiamo aggiunto --root: questo è il segreto per far andare Nmap e VPN
distrobox create --image docker.io/kalilinux/kali-rolling --name kali --root -Y

# 2. Install Kali Headless metapackage
echo "🚀 Installing Kali Headless..."

# Non serve 'sudo' dentro il comando bash -c perché siamo già root
distrobox enter --root kali -- bash -c "
    apt-get update && \
    apt-get install -y kali-linux-headless && \
    apt-get clean
"

echo "✅ Done! To enter your Kali environment, type: distrobox enter --root kali"
