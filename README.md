# NullVoidOS &nbsp; [![bluebuild build badge](https://github.com/daenaihax/nullvoidos/actions/workflows/build.yml/badge.svg)](https://github.com/daenaihax/nullvoidos/actions/workflows/build.yml)

> ⚠️ **Experimental / Testing Phase**
>
> NullVoidOS is currently under active development and testing.
> Features, workflows, and interfaces may change or break.
> This project is intended for testing, experimentation, and early adopters only.

<img width="1280" height="800" alt="Desktop" src="https://github.com/user-attachments/assets/bf59068f-0f95-4d83-bbd3-f249d6b4bd68" />


## Mission

NullVoidOS aims to provide a unified, container-centric cybersecurity workbench
that simplifies complex security workflows without sacrificing native tooling
or operational control.

The project focuses on:

- Consolidating offensive, forensic, and incident response toolsets into a single environment
- Reducing reliance on multiple heavyweight virtual machines
- Preserving native workflows and existing muscle memory
- Treating the host system as immutable infrastructure, not a workspace
- Enabling reproducible, resettable security environments for professional use

## Design Philosophy

NullVoidOS is built around a small set of explicit design principles:

- Containers are used to organize toolsets, not to hide them
- Native workflows are preferred over abstraction layers
- The host system is treated as infrastructure, not as a working environment
- Security domains are isolated by design, not by convention
- Reproducibility and resetability are first-class concerns
- Convenience must not weaken the host security model

## Architecture (High-Level)

NullVoidOS follows a layered architecture with clear separation of concerns:

- A Fedora Atomic–based immutable host system acting as infrastructure
- Root-enabled containers providing isolated security toolsets
- Host-level orchestration for lifecycle management and updates
- VM-first deployment model to reduce risk and simplify recovery

## Scope and Limitations

NullVoidOS is intentionally scoped to a specific set of use cases and
operational assumptions.

### In Scope
- Professional cybersecurity workflows (offensive security, DFIR, incident response)
- Expert users comfortable with terminal-based environments
- VM-based or dedicated hardware deployments
- Disposable and resettable working environments

### Out of Scope
- Daily desktop usage
- Personal computing or general-purpose workloads
- Beginner-friendly abstractions or guided workflows
- Strong isolation against a malicious local user

## Installation & Usage

NullVoidOS is installed by rebasing an existing Fedora Atomic system.

The installation process provides only the immutable base system.
All security environments and toolsets are provisioned explicitly
by the user after installation.

### Documentation

- **[Installation Guide](docs/INSTALLATION.md)**  
  Base system installation and rebase instructions.

- **[Commands Reference](docs/COMMANDS.md)**  
  User-facing commands for provisioning security environments,
  command resolution, privileged execution, and container lifecycle management.

## Upstream Documentation

NullVoidOS is built on top of Fedora Atomic technologies and relies on
upstream tooling for system management and updates.

For detailed documentation on the underlying platform, refer to:

- Fedora Atomic Desktops documentation  
  https://docs.fedoraproject.org/en-US/fedora-silverblue/

- rpm-ostree documentation  
  https://coreos.github.io/rpm-ostree/

- Distrobox documentation  
  https://distrobox.it/

These resources describe system-level commands and workflows that are
outside the scope of NullVoidOS-specific tooling.

