## Environment Initialization

NullVoidOS does not ship with security toolsets preinstalled.

To provision the available environments, dedicated initialization commands
are provided.

The following commands are used to install the Kali and SIFT containers:

- `init-kali` – installs the Kali Linux offensive security environment
- `init-sift` – installs the SIFT forensic analysis environment


## Command Resolution

NullVoidOS provides transparent command resolution across the host system
and containerized toolsets.

Commands are not installed globally on the host. Instead, execution is
routed to the appropriate environment when required.

### Resolution Order

When a command is executed from the host shell, the following resolution
process is applied:

1. **Host check**  
   If the command exists on the host system, it is executed locally.

2. **Container fallback**  
   If the command does not exist on the host, the system searches available
   tool containers for a matching executable.

3. **Execution routing**  
   If a container provides the command, execution is transparently routed
   to that container.

4. **Caching**  
   The resolved container is cached to avoid repeated discovery on
   subsequent executions.

### Example

```
nmap -h
```

Container Kali
<img width="1280" height="800" alt="kali_nmap" src="https://github.com/user-attachments/assets/1f2690f8-cbb2-4e7e-b93b-65a3bb5d2f60" />


Host
<img width="1280" height="800" alt="host_nmap" src="https://github.com/user-attachments/assets/73b24849-effe-4c37-bbaa-482028b5104f" />

## nvx

`nvx` is a privileged execution helper designed to integrate with the
NullVoidOS command resolution model.

It provides a sudo-like interface while preserving container-based
execution and host security boundaries.

### Why nvx exists

In a container-centric environment, privileged commands present a challenge:

- Some tools require root privileges
- Tools may reside inside containers rather than on the host
- Manually switching execution contexts interrupts workflows

`nvx` addresses this by combining privilege escalation with automatic
execution routing.

### Behavior

When a command is executed using `nvx`, the following logic is applied:

1. **Host resolution**  
   If the command exists on the host system, it is executed on the host
   with elevated privileges.

2. **Container fallback**  
   If the command does not exist on the host, it is resolved using the
   same container discovery mechanism described in the command resolution
   model.

3. **Privileged execution**  
   If the command is executed inside a container, privileges are elevated
   within the container context, not on the host.


### Privileged tools example

Some tools require elevated privileges by design and cannot function
without root access.

A common example is `airmon-ng`, which manages wireless interfaces and
requires direct access to network drivers.

```bash
$ which airmon-ng
airmon-ng not found

$ airmon-ng
Run it as root

$ nvx airmon-ng
```

Container Kali
<img width="1283" height="802" alt="kali_airmon" src="https://github.com/user-attachments/assets/547e7f0b-2a5e-48c6-ab98-2b80595376d2" />
Host
<img width="1283" height="802" alt="host_airmon" src="https://github.com/user-attachments/assets/392cc3c2-de3c-4e27-ac63-3168c8130378" />



## dbx-destroy

Interactive container removal tool for Distrobox environments.

This command is used to destroy one or more toolset containers managed by
NullVoidOS. Containers are treated as disposable and can be safely removed
and recreated at any time.

```bash
dbx-destroy
```

Host
![dbx-destroy](docs/assets/dbx-destroy.png)


```bash
dbx-destroy --all
```

This option removes all Distrobox containers on the system, including both  
rootless and rootful containers.

This operation is destructive and cannot be undone.
### Intended usage

Containers in NullVoidOS are not considered long-lived environments.

Destroying and recreating containers is a normal and expected workflow,
used to:

- Reset compromised environments
- Recover from configuration drift
- Reproduce clean analysis conditions
- Free system resources

Toolset provisioning is handled by dedicated initialization commands,
making container removal safe and reversible.

### Security note

Container removal does not affect the host system.

No tools are uninstalled from the host, and no system state is modified
outside of the container runtime.

## Final notes

NullVoidOS commands are designed to support expert workflows without hiding
execution context or introducing implicit behavior.

Users are expected to understand the implications of privileged execution
and container lifecycle management.
