# PINDOS Makefile — Linux

TARGET     = i686-unknown-none
KERNEL_ELF = target/kernel.elf
KERNEL_BIN = target/kernel.bin
BOOT_BIN   = target/boot.bin
OS_IMG     = target/pindos.img

NASM  = nasm
QEMU  = qemu-system-i386
CARGO = cargo

# Ищем rust-lld и llvm-objcopy в nightly GNU toolchain
NIGHTLY_SYSROOT := $(shell rustup run nightly-x86_64-pc-windows-gnu rustc --print sysroot)
TOOLCHAIN_BIN   := $(NIGHTLY_SYSROOT)/lib/rustlib/x86_64-pc-windows-gnu/bin
LLD             := $(TOOLCHAIN_BIN)/rust-lld
OBJCOPY         := $(TOOLCHAIN_BIN)/llvm-objcopy

.PHONY: all clean run debug check-tools

all: check-tools font $(OS_IMG)

font:
	python3 scripts/gen_font.py

check-tools:
	@which $(NASM) > /dev/null 2>&1 \
		|| (echo "ERROR: nasm not found — sudo apt install nasm" && exit 1)
	@which $(QEMU) > /dev/null 2>&1 \
		|| (echo "ERROR: qemu not found — sudo apt install qemu-system-x86" && exit 1)
	@rustup run nightly cargo --version > /dev/null 2>&1 \
		|| (echo "ERROR: rust nightly not found — rustup install nightly" && exit 1)
	@test -f "$(LLD)" \
		|| (echo "ERROR: rust-lld not found — rustup component add llvm-tools-preview --toolchain nightly" && exit 1)
	@echo "[OK] All tools found"
	@echo "     LLD:     $(LLD)"
	@echo "     OBJCOPY: $(OBJCOPY)"

target:
	mkdir -p target

# ── Загрузчик ──────────────────────────────────────────────────────────────
$(BOOT_BIN): bootloader/boot.asm | target
	$(NASM) -f bin bootloader/boot.asm -o $(BOOT_BIN)

# ── Rust ядро ──────────────────────────────────────────────────────────────
target/libhammam.a: $(shell find kernel/src -name '*.rs') | target
	cd kernel && rustup run nightly-x86_64-pc-windows-gnu cargo build --release
	cp kernel/target/$(TARGET)/release/libhammam.a target/

# ── ASM точка входа ────────────────────────────────────────────────────────
target/kernel_entry.o: kernel/kernel.asm | target
	$(NASM) -f elf32 kernel/kernel.asm -o target/kernel_entry.o

# ── Линковка ───────────────────────────────────────────────────────────────
$(KERNEL_ELF): target/kernel_entry.o target/libhammam.a linker.ld
	"$(LLD)" -flavor ld -m elf_i386 \
		-T linker.ld \
		--gc-sections \
		target/kernel_entry.o \
		target/libhammam.a \
		-o $(KERNEL_ELF)

# ── Flat binary ────────────────────────────────────────────────────────────
$(KERNEL_BIN): $(KERNEL_ELF)
	"$(OBJCOPY)" -O binary $(KERNEL_ELF) $(KERNEL_BIN)
	@echo "Kernel size: $$(wc -c < $(KERNEL_BIN)) bytes"

# ── Образ диска ────────────────────────────────────────────────────────────
$(OS_IMG): $(BOOT_BIN) $(KERNEL_BIN)
	cat $(BOOT_BIN) $(KERNEL_BIN) > $(OS_IMG)
	python3 -c "\
import os; \
f = open('$(OS_IMG)', 'ab'); \
s = os.path.getsize('$(OS_IMG)'); \
pad = 1474560 - s; \
f.write(b'\x00' * pad if pad > 0 else b''); \
f.close(); \
print('Image: %d bytes' % os.path.getsize('$(OS_IMG)'))"

# ── Запуск ─────────────────────────────────────────────────────────────────
run: $(OS_IMG)
	$(QEMU) \
		-drive format=raw,file=$(OS_IMG),if=floppy \
		-m 32M \
		-display sdl \
		-no-reboot

# ── Запуск через GRUB/Multiboot2 (как на реальном железе с UEFI) ───────────
run-grub: $(KERNEL_ELF)
	$(QEMU) \
		-kernel $(KERNEL_ELF) \
		-m 64M \
		-display sdl \
		-no-reboot

# ── ISO образ (BIOS + UEFI через GRUB) ────────────────────────────────────
iso: $(KERNEL_ELF)
	bash scripts/make_usb.sh --iso

# ── Гибридный IMG ─────────────────────────────────────────────────────────
img: $(KERNEL_ELF)
	bash scripts/make_usb.sh --img

# ── Отладка ────────────────────────────────────────────────────────────────
debug: $(OS_IMG)
	$(QEMU) \
		-drive format=raw,file=$(OS_IMG),if=floppy \
		-m 32M \
		-s -S \
		-no-reboot &
	sleep 1
	gdb \
		-ex "set architecture i386" \
		-ex "target remote :1234" \
		-ex "symbol-file $(KERNEL_ELF)" \
		-ex "break _start" \
		-ex "continue"

# ── Отладка через GRUB ────────────────────────────────────────────────────
debug-grub: $(KERNEL_ELF)
	$(QEMU) \
		-kernel $(KERNEL_ELF) \
		-m 64M \
		-s -S \
		-no-reboot &
	sleep 1
	gdb \
		-ex "set architecture i386" \
		-ex "target remote :1234" \
		-ex "symbol-file $(KERNEL_ELF)"

clean:
	rm -rf target
	cd kernel && $(CARGO) clean
