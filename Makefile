.PHONY: build build-kernel iso clean

KERNEL_DEBUG = hammam/target/x86_64-unknown-none/debug/hammam-kernel
KERNEL_CLEAN = hammam/target/x86_64-unknown-none/debug/hammam-kernel.clean

build-kernel:
	cd hammam && cargo build --target x86_64-unknown-none
	@echo "Fixing ELF OS/ABI for baremetal..."
	@# Set OSABI to 0 (NONE) for baremetal ELF
	@if command -v llvm-objcopy &> /dev/null; then \
		llvm-objcopy --set-osabi=unix $(KERNEL_DEBUG); \
	elif command -v objcopy &> /dev/null; then \
		objcopy --set-osabi=unix $(KERNEL_DEBUG); \
	else \
		echo "WARNING: objcopy not found, OSABI might not be correct"; \
	fi

iso: build-kernel
	wsl bash tools/make_iso.sh

clean:
	cd hammam && cargo clean
	rm -rf iso_root hammam.iso

all: iso

