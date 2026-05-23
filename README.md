# PINDOS - Production Microkernel Operating System

PINDOS is a production-ready, secure microkernel-based operating system written entirely in Rust. It consists of two main components:

- **Hammam**: A secure, high-performance microkernel
- **dealduck**: A native init system and service manager (systemd alternative)
- **dealdo**: CLI control tool for dealduck

## Architecture Overview

### Hammam Microkernel

Hammam implements a minimalist microkernel design where only the following run in Ring 0 (Kernel Space):

1. **Address Space Management** - Virtual memory mapping and page tables
2. **Thread Scheduling** - Priority-based preemptive scheduler with DAG-based ready queues
3. **Inter-Process Communication (IPC)** - Capability-based message passing with endpoints

All drivers (timer, keyboard, serial, storage, network) run in Ring 3 (User Space) as separate processes communicating via IPC.

### Key Features

- **Dual Architecture Support**: x86_64 (AMD64) and ARM64 (AArch64)
- **GRUB Bootloader**: Multiboot2 compliant for x86_64, UEFI/GRUB for ARM64
- **Capability-Based Security**: Endpoint-based IPC with proper capability tracking
- **Syscall Redirection Layer**: Traps POSIX syscalls and forwards to user-space servers (VFS, Network)
- **Physical Memory Management**: Bitmap-based frame allocator supporting up to 256GB
- **Virtual Memory**: Full paging support with 4KB pages, higher half kernel mapping

### dealduck Init System

A systemd-compatible init system with:

- **INI-style Unit Parser**: Reads actual systemd `.service` files
- **Dependency Resolution**: DAG-based topological sorting for parallel startup
- **Service State Machine**: Full state tracking (Dead → Starting → Running → Stopping → Failed)
- **Restart Policies**: Support for all systemd restart policies

### dealdo CLI

Command-line tool with full systemd-compatible syntax:

```bash
sudo dealdo start nginx.service
sudo dealdo stop docker.socket
sudo dealdo restart network.service
sudo dealdo status --full
sudo dealdo journal -u nginx -f
sudo dealdo system-status
```

## Project Structure

```
pindos/
├── Cargo.toml                    # Workspace configuration
├── kernel/                       # Hammam microkernel
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # Kernel entry point
│   │   ├── arch/                # Architecture-specific code
│   │   │   ├── mod.rs
│   │   │   ├── x86_64/          # x86_64 implementation
│   │   │   │   ├── mod.rs
│   │   │   │   ├── bootstrap.rs
│   │   │   │   ├── gdt.rs
│   │   │   │   ├── idt.rs
│   │   │   │   └── paging.rs
│   │   │   └── aarch64/         # ARM64 implementation
│   │   │       └── mod.rs
│   │   ├── scheduler/           # Thread/process scheduler
│   │   ├── memory/              # Memory management
│   │   ├── ipc/                 # IPC subsystem
│   │   └── syscall/             # System call interface
│   ├── asm/
│   │   ├── boot_x86_64.S        # x86_64 boot assembly
│   │   └── boot_aarch64.S       # ARM64 boot assembly
│   ├── linker_x86_64.ld         # x86_64 linker script
│   └── linker_aarch64.ld        # ARM64 linker script
├── crates/
│   ├── memory/                  # Memory management crate
│   ├── scheduler/               # Scheduler crate
│   └── ipc/                     # IPC crate
├── user/
│   ├── dealduck/                # Init system
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── unit_parser.rs
│   │       ├── dependency_graph.rs
│   │       └── service_state.rs
│   └── dealdo/                  # CLI tool
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── docs/
    └── design_code/             # Design documentation
```

## Building

### Prerequisites

- Rust nightly with `x86_64-unknown-none` and `aarch64-unknown-none` targets
- GNU Assembler (gas)
- GRUB 2.0+
- QEMU for testing

### Build Commands

```bash
# Build kernel for x86_64
cargo build --release --target x86_64-unknown-none

# Build kernel for ARM64
cargo build --release --target aarch64-unknown-none

# Build dealduck init system
cd user/dealduck && cargo build --release

# Build dealdo CLI
cd user/dealdo && cargo build --release
```

### Creating Bootable Image

```bash
# Create GRUB-compatible ISO
grub-mkrescue -o pindos.iso iso/
```

## Boot Process

### x86_64 (GRUB Multiboot2)

1. BIOS/UEFI loads GRUB
2. GRUB loads kernel ELF image
3. GRUB passes Multiboot2 info structure
4. Assembly code sets up GDT, IDT, enables paging
5. Rust `kmain()` initializes:
   - Physical frame allocator
   - Page tables
   - Scheduler
   - IPC subsystem
   - System call interface
6. Spawns dealduck as PID 1

### ARM64 (UEFI/GRUB)

1. UEFI firmware loads GRUB
2. GRUB passes Device Tree Blob (DTB)
3. Assembly code sets up exception vectors, enables MMU
4. Rust `kmain()` parses DTB for memory regions
5. Same initialization as x86_64

## System Calls

Hammam provides a comprehensive syscall interface:

| Number | Name | Description |
|--------|------|-------------|
| 0 | Exit | Terminate process |
| 1 | Fork | Create child process |
| 2 | Exec | Replace process image |
| 3 | Wait | Wait for child |
| 4 | Brk | Set data segment size |
| 5 | Mmap | Map memory region |
| 6 | Munmap | Unmap memory |
| 7 | Send | IPC send |
| 8 | Recv | IPC receive |
| 20 | GetPid | Get process ID |
| 21 | GetTid | Get thread ID |
| 22 | Yield | Yield CPU |
| 30-39 | File ops | Open, Close, Read, Write, etc. |
| 40 | Socket | Create socket |
| 50-52 | Shm | Shared memory operations |

## IPC Mechanism

Hammam uses a capability-based IPC system:

- **Endpoints**: Communication channels identified by unique IDs
- **Messages**: Up to 256 bytes inline, with sender/receiver/priority
- **Queues**: Per-endpoint message queues (max 16 messages)
- **Blocking/Non-blocking**: Configurable per endpoint

### Example IPC Usage

```rust
// Create endpoint
let endpoint = ipc::create_endpoint(pid, EndpointType::Server, EndpointFlags::BLOCKING);

// Send message
ipc::sys_send(receiver_id, b"Hello", MessagePriority::Normal);

// Receive message
let mut buffer = [0u8; 256];
let len = ipc::sys_recv(endpoint_id, &mut buffer, true)?;
```

## Memory Management

### Physical Frame Allocator

- Bitmap-based allocation
- Supports up to 256GB physical memory
- 4KB frame size
- Reserved regions for kernel, BIOS, etc.

### Virtual Memory

- 4KB pages with 4-level paging (x86_64)
- Higher half kernel mapping (0xFFFF800000000000)
- User space from 0x0 to user space limit
- Copy-on-write support for fork()

## Scheduler

### Priority-Based Preemptive Scheduling

- 5 priority levels: Idle, Low, Normal, High, Critical
- Round-robin within same priority
- Time slice based preemption
- DAG-based ready queues

### Thread Context

Full register save/restore on context switch:
- General purpose: RAX, RBX, RCX, RDX, RSI, RDI, RBP, R8-R15
- Special: RIP, RSP, RFLAGS
- Segment registers reloaded on each switch

## Security Model

### Capability-Based Access

- Endpoints are capabilities
- Processes can only communicate via endpoints they hold
- No direct memory access between processes

### Principle of Least Privilege

- Drivers run in Ring 3 (user space)
- Minimal kernel code in Ring 0
- All hardware access mediated by user-space drivers

## License

MIT License

## Contributing

See the contributing guidelines for more information.