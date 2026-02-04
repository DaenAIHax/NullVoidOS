# Environment Setup

This document describes how a NullVoidOS installation is prepared for
day-to-day security work after the base system has been installed.

NullVoidOS does not ship with security toolchains installed on the host.
Instead, all tooling is provisioned inside dedicated containers.

---

## Post-Installation State

After installing NullVoidOS, the system is intentionally minimal:

- No offensive or forensic tools are installed on the host
- The host system is treated as immutable infrastructure
- No working environment is available until toolsets are initialized

This is expected behavior.

---

## Toolset Initialization Model

Security toolsets are provisioned using dedicated initialization scripts.
Each script is responsible for setting up a complete execution environment
for a specific security domain.

These scripts are designed to be:

- Idempotent
- Re-runnable
- Safe to execute after container removal

Destroying and recreating containers is considered a normal workflow.

---

## Kali Linux Environment

This environment is based on Kali Linux.


It is provisioned using a dedicated initialization script that:

- Creates a Kali Linux container
- Installs the headless Kali toolchain
- Configures the container for privileged execution
- Prepares the environment for interactive terminal usage

The Kali container is intended for tasks such as:

- Network enumeration
- Exploitation
- Post-exploitation
- Red team and adversary emulation activities

For detailed documentation about Kali tools and usage, refer to the
official Kali Linux documentation:

https://www.kali.org/docs/containers/official-kalilinux-docker-images/


---

## SIFT Workstation

This environment is based on the official SIFT Workstation maintained by Team DFIR.

It provides a comprehensive set of forensic analysis tools, including:

- memory analysis
- file system forensics
- timeline analysis
- network capture analysis
- and many others

For full documentation on the tools and exercises available in SIFT,
see the official repository:

https://github.com/teamdfir/sift

---

## Execution Context

Each toolset is isolated within its own container and is accessed
through its native environment.

Users are expected to select the appropriate execution context
based on the task being performed, rather than installing tools
globally on the host system.

---

## Reproducibility and Lifecycle

Toolset containers are treated as disposable artifacts.

A typical lifecycle includes:

1. Provisioning the environment using an initialization script
2. Performing security work
3. Destroying the container when no longer needed
4. Recreating it when a clean state is required

This model improves reproducibility and reduces long-term system drift.
