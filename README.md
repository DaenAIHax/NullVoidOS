# NullVoidOS &nbsp; [![bluebuild build badge](https://github.com/daenaihax/nullvoidos/actions/workflows/build.yml/badge.svg)](https://github.com/daenaihax/nullvoidos/actions/workflows/build.yml)

A modular cybersecurity workbench

## Overview

This project is a **modular cybersecurity workbench** designed to provide a **single, consistent operating environment** for:

- offensive security
- DFIR
- incident response activities

### Core idea

Instead of maintaining multiple heavy virtual machines (Kali, SIFT, REMnux, etc.), the workbench uses:

- **one immutable base system**
- **specialized toolsets running in rootful containers**
- **direct access via terminal profiles**

The goal is to reduce operational complexity while preserving native workflows.

### Usage assumptions

This workbench is **not meant to be used as a daily desktop OS**.

It is designed to:

- run **inside a virtual machine**
- or from **dedicated external storage** (e.g. NVMe over USB)
- be treated as a **work-only, disposable environment**

---

## Design Goals

- One operating system, multiple security domains
- No need to run multiple VMs at the same time
- Native tool usage (no wrappers around every command)
- Clear separation between base system and toolsets
- Suitable for pentesting, DFIR, and incident response
- Reproducible and resettable setup

> This project does **not** try to replace Kali Linux or SIFT.  
> It provides a workspace that **includes them**, in a way that scales better operationally.

---

## Architecture

### Base System

- Fedora Atomic–based system (immutable host)
- Minimal desktop environment
- Host treated as **infrastructure**, not as a working environment

### Toolsets (Containers)

The workbench currently includes two main containers.

#### Kali Linux (Offensive Security)

- Kali Linux Headless installation
- Full offensive toolset (~2900 tools)
- Rootful container
- Host-networked

Used for:
- network enumeration
- exploitation
- lateral movement
- post-exploitation

#### SIFT Workstation (DFIR)

- Official SIFT container image
- Rootful container

Used for:
- disk and memory analysis
- forensic triage
- incident response activities

Both containers are accessed **directly via terminal profiles**, not via command wrappers.

---

## Usage Model

The workbench is designed for **expert workflows**.

Instead of abstracting tools behind a launcher, the user:

1. Opens the terminal selector
2. Chooses the appropriate container (Kali or SIFT)
3. Works **natively inside that environment**

### Example

- Open Kali terminal → run `nmap`, `gobuster`, `crackmapexec`
- Open SIFT terminal → run `bulk_extractor`, `volatility`, forensic tools

This preserves:

- muscle memory
- existing workflows
- compatibility with long and complex command lines

No additional command prefix is required.

---

## Why Rootful Containers

Both containers run as **rootful by design**.

This is a conscious and explicit trade-off.

- **Kali** requires privileged access to networking features  
  (raw sockets, tunneling, scanning)
- **SIFT** requires privileged access for memory and forensic operations

### Security considerations

- The system is **not used on bare metal**
- It runs inside a VM or on dedicated external storage
- No personal data or credentials are expected on the host
- The environment is considered **sacrificial and resettable**

Atomic updates and rollback further reduce operational risk.

---

## Scope and Limitations

### In scope

- Unified security workspace
- Offensive + defensive tooling in one environment
- Professional, expert-level usage
- VM-first deployment

### Out of scope

- Daily desktop usage
- Personal browsing or private data
- Beginner-friendly abstractions
- Strong isolation against a malicious local user

---

## Current State

At the moment, the project provides:

- Base Atomic system
- Two installation scripts:
  - Kali container (Kali Headless)
  - SIFT container (official image)
- Terminal-based access to both containers
- Rootful execution model

This is the **core foundation** of the workbench.

Future improvements (tool discovery, orchestration helpers, manifests) will build on top of this base **without breaking the native workflow**.

---

## Philosophy

> Use containers to **organize toolsets**, not to hide them.  
> Use the host as **infrastructure**, not as a workstation.  
> Optimize for professionals who need **one environment that can do everything**.

See the [BlueBuild docs](https://blue-build.org/how-to/setup/) for quick setup instructions for setting up your own repository based on this template.

After setup, it is recommended you update this README to describe your custom image.

## Installation

> [!WARNING]  
> [This is an experimental feature](https://www.fedoraproject.org/wiki/Changes/OstreeNativeContainerStable), try at your own discretion.

To rebase an existing atomic Fedora installation to the latest build:

- First rebase to the unsigned image, to get the proper signing keys and policies installed:
  ```
  rpm-ostree rebase ostree-unverified-registry:ghcr.io/daenaihax/nullvoidos:latest
  ```
- Reboot to complete the rebase:
  ```
  systemctl reboot
  ```
- Then rebase to the signed image, like so:
  ```
  rpm-ostree rebase ostree-image-signed:docker://ghcr.io/daenaihax/nullvoidos:latest
  ```
- Reboot again to complete the installation
  ```
  systemctl reboot
  ```

The `latest` tag will automatically point to the latest build. That build will still always use the Fedora version specified in `recipe.yml`, so you won't get accidentally updated to the next major version.

## ISO

If build on Fedora Atomic, you can generate an offline ISO with the instructions available [here](https://blue-build.org/learn/universal-blue/#fresh-install-from-an-iso). These ISOs cannot unfortunately be distributed on GitHub for free due to large sizes, so for public projects something else has to be used for hosting.

## Verification

These images are signed with [Sigstore](https://www.sigstore.dev/)'s [cosign](https://github.com/sigstore/cosign). You can verify the signature by downloading the `cosign.pub` file from this repo and running the following command:

```bash
cosign verify --key cosign.pub ghcr.io/daenaihax/nullvoidos
```
