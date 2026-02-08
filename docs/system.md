## Architecture

NullVoidOS is built around a transparent command orchestration layer,
designed to seamlessly integrate the host system with containerized
environments.

The goal is simple:
the user runs commands, the system resolves where they belong.

### Command Resolution Model

At the core of the system is an automatic command resolution mechanism.

When a command is executed:
1. The system first checks if it exists on the host.
2. If not found, it searches inside rootful Distrobox containers.
3. If the command is located, it is transparently executed in the correct environment.

### DBX Command Layer (internal)

This behavior is implemented by an internal dispatcher based on `dbx-cmd`.

This layer:
- scans available containers
- detects command availability
- executes the command in the appropriate context
- caches command-to-container mappings to avoid repeated scans

The user never interacts with this layer directly.

<img width="1377" height="857" alt="image" src="https://github.com/user-attachments/assets/46e7fb0a-8d54-47fd-9a96-d907f532ef13" />



### Command Caching

To improve performance, resolved commands are cached locally.

Once a command is associated with a container, subsequent executions
are performed instantly without re-scanning all environments.

### Privilege Model

Only rootful containers are used for command execution.

This design choice allows:
- full network access
- raw socket usage
- low-level tooling (offensive and forensic)

### Privileged Command Execution

Commands executed through the orchestration layer are run with elevated
privileges when required.

```bash
nvx <command>
```
<img width="1377" height="857" alt="image" src="https://github.com/user-attachments/assets/fc02d0cc-c834-45e4-afc2-88afe60e81eb" />


The `nvx` wrapper enforces a privileged execution model:
- commands are executed via `sudo` on the host
- commands executed inside containers are invoked with equivalent privileges

This ensures consistent behavior between host and container environments,
especially for tooling that requires administrative access.

Privilege escalation is explicit and controlled.
The system does not silently bypass host security mechanisms.

### Design Philosophy

The architecture follows a few core principles:

- **Transparency**: commands behave as if they were installed locally
- **Isolation**: tools live in containers, not on the host
- **Immutability**: the host system remains clean and reproducible
- **Disposability**: containers are environments, not pets

---

## User Commands

NullVoidOS does not ship with offensive or forensic tools on the host system.
All tooling lives inside containers.

The system supports two operational models:
- **Prebuilt environments**, for users who want a ready-to-use workstation.
- **Custom environments**, for advanced users managing their own toolchains.

### Environment Bootstrap

NullVoidOS provides optional commands to create complete, preconfigured
containerized environments.

These environments are intended to provide a workflow comparable to
traditional security-focused operating systems.

#### Kali Linux (headless)

```bash
init-kali
```

- Rootful Distrobox container
- Intended for offensive security training and operations

#### SIFT Workstation

```bash
init-sift
```

- Rootful Distrobox container
- Intended for DFIR and malware analysis

All tools are installed inside containers.
The host system remains unchanged.

Prebuilt environments are disposable and not part of the base system.

### Custom Environments

Users are not required to use the provided bootstrap environments.

Any Distrobox container can be created manually and populated with a
custom toolset.

NullVoidOS does not assume a specific distribution or tooling.
Only the interaction model is provided.

### Tool Management

Once an environment exists, tools can be managed using the following commands:

```bash
nvx-where <command>
```
locate a tool across host and containers

<img width="1377" height="857" alt="image" src="https://github.com/user-attachments/assets/09f77cb2-cdbf-4a93-8d69-6f104ed2f620" />



```bash
nvx-install <package>
```

install a package inside a selected container

```bash
nvx remove <package>
```

remove a package from a container

### System Update

Update the entire system stack:

- host (rpm-ostree)
- Flatpaks
- containers

Command:

```bash
nvx-update
```

---

## Security Model

NullVoidOS supports two distinct security profiles, intended to adapt
the system to different operational contexts.

Security configuration is managed through a single control interface.


### NVX Shield

The system security profile is controlled via `nvx-shield`.

Usage:

`nvx-shield on`

`nvx-shield off`

`nvx-shield status`

The command manages the following components:
- SELinux enforcement mode
- firewalld service state

### Operating Modes

#### Secure Mode

Activated with:

```bash
nvx-shield on
```

- SELinux: enforcing
- firewalld: enabled

Intended for daily use, development, and general-purpose operation.

#### Offensive Mode

Activated with:

```bash
nvx-shield off
```

- SELinux: permissive
- firewalld: disabled

Intended for offensive security, lab environments, and tooling that
requires unrestricted network or low-level system access.

### Status Inspection

Current security state can be queried using:

```bash
nvx-shield status
```

### Scope and Limitations

NVX Shield only affects the host system.

Container-level security and isolation are managed independently by
the container runtime and the container images.

---

## Environments & Containers

Containers in NullVoidOS are considered disposable environments.

They can be created, replaced, or destroyed without affecting the host
system or other environments.

### DBX Destroy

Container removal is managed through the `destroy-dbx` command.

This command allows:
- removing individual containers
- removing all containers (rootful and rootless)
- forced removal when required

Usage:
```bash
dbx-destroy
```
```bash
dbx-destroy --all
```
```bash
dbx-destroy --force
```
This operation permanently deletes the selected containers.
The host system and orchestration layer are not affected.



