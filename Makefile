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
HOST_TRIPLE     := $(shell rustc -vV 2>/dev/null | sed -n 's/^host: //p')
NIGHTLY_SYSROOT := $(shell rustup run nightly rustc --print sysroot 2>/dev/null)
TOOLCHAIN_BIN   := $(NIGHTLY_SYSROOT)/lib/rustlib/$(HOST_TRIPLE)/bin
LLD             := $(TOOLCHAIN_BIN)/rust-lld
OBJCOPY         := $(TOOLCHAIN_BIN)/llvm-objcopy

.PHONY: all clean run run-direct run-serial run-grub-serial debug check-tools

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
	@test -n "$(HOST_TRIPLE)" \
		|| (echo "ERROR: rustc host triple not detected" && exit 1)
	@test -f "$(LLD)" \
		|| (echo "ERROR: rust-lld not found — rustup component add llvm-tools-preview --toolchain nightly" && exit 1)
	@echo "[OK] All tools found"
	@echo "     HOST:    $(HOST_TRIPLE)"
	@echo "     LLD:     $(LLD)"
	@echo "     OBJCOPY: $(OBJCOPY)"

target:
	mkdir -p target

# ── Загрузчик ──────────────────────────────────────────────────────────────
$(BOOT_BIN): bootloader/boot.asm | target
	$(NASM) -f bin bootloader/boot.asm -o $(BOOT_BIN)

# ── Rust ядро ──────────────────────────────────────────────────────────────
target/libpindos_kernel.a: $(shell find kernel/src -name '*.rs') | target
	cd kernel && rustup run nightly cargo build --release
	cp kernel/target/$(TARGET)/release/libpindos_kernel.a target/

# ── ASM точка входа ────────────────────────────────────────────────────────
target/kernel_entry.o: kernel/kernel.asm | target
	$(NASM) -f elf32 kernel/kernel.asm -o target/kernel_entry.o

# ── Линковка ───────────────────────────────────────────────────────────────
$(KERNEL_ELF): target/kernel_entry.o target/libpindos_kernel.a linker.ld
	"$(LLD)" -flavor ld -m elf_i386 \
		-T linker.ld \
		--gc-sections \
		target/kernel_entry.o \
		target/libpindos_kernel.a \
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
		-vga std \
		-display sdl \
		-no-reboot

# ── Прямой запуск без ISO/GRUB ────────────────────────────────────────────
run-direct: run

run-serial: $(OS_IMG)
	$(QEMU) \
		-drive format=raw,file=$(OS_IMG),if=floppy \
		-m 32M \
		-vga std \
		-display none \
		-serial stdio \
		-no-reboot

# ── Настоящий запуск через GRUB/ISO ───────────────────────────────────────
run-grub: iso
	$(QEMU) \
		-cdrom target/pindos.iso \
		-m 256M \
		-vga std \
		-display sdl \
		-audiodev pa,id=snd0 \
		-machine pcspk-audiodev=snd0 \
		-no-reboot

run-grub-serial: iso
	$(QEMU) \
		-cdrom target/pindos.iso \
		-m 256M \
		-vga std \
		-display none \
		-serial stdio \
		-no-reboot

# ── Прямой запуск ядра через qemu -kernel (экспериментальный путь) ───────
run-kernel: $(KERNEL_ELF)
	$(QEMU) \
		-kernel $(KERNEL_ELF) \
		-m 64M \
		-vga std \
		-display sdl \
		-no-reboot

# ── Явный bootloader/floppy path ───────────────────────────────────────────
run-boot: $(OS_IMG)
	$(QEMU) \
		-drive format=raw,file=$(OS_IMG),if=floppy \
		-m 32M \
		-vga std \
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
		-vga std \
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
		-vga std \
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
