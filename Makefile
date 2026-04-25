# PINDOS Makefile — Linux

TARGET     = i686-unknown-none
KERNEL_ELF = target/kernel.elf
KERNEL_BIN = target/kernel.bin
BOOT_BIN   = target/boot.bin
OS_IMG     = target/pindos.img

NASM    = nasm
QEMU    = qemu-system-i386
CARGO   = cargo

RUST_SYSROOT := $(shell cargo +nightly rustc -- --print sysroot 2>/dev/null)
LLD      = $(RUST_SYSROOT)/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld
OBJCOPY  = $(RUST_SYSROOT)/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-objcopy

.PHONY: all clean run debug check-tools

all: check-tools $(OS_IMG)

check-tools:
	@which $(NASM)  > /dev/null 2>&1 || (echo "ERROR: nasm not found.  sudo apt install nasm" && exit 1)
	@which $(QEMU)  > /dev/null 2>&1 || (echo "ERROR: qemu not found.  sudo apt install qemu-system-x86" && exit 1)
	@cargo +nightly --version > /dev/null 2>&1 || (echo "ERROR: rust nightly not found. rustup install nightly" && exit 1)
	@test -f "$(LLD)" || (echo "ERROR: rust-lld not found. rustup component add llvm-tools-preview --toolchain nightly" && exit 1)
	@echo "[OK] All tools found"

target:
	mkdir -p target

# Загрузчик
$(BOOT_BIN): bootloader/boot.asm | target
	$(NASM) -f bin bootloader/boot.asm -o $(BOOT_BIN)

# Rust ядро
target/libpindos_kernel.a: kernel/src/main.rs | target
	cd kernel && $(CARGO) +nightly build --release
	cp kernel/target/$(TARGET)/release/libpindos_kernel.a target/

# ASM точка входа
target/kernel_entry.o: kernel/kernel.asm | target
	$(NASM) -f elf32 kernel/kernel.asm -o target/kernel_entry.o

# Линковка
$(KERNEL_ELF): target/kernel_entry.o target/libpindos_kernel.a
	$(LLD) -flavor ld -m elf_i386 -T linker.ld \
		target/kernel_entry.o \
		target/libpindos_kernel.a \
		-o $(KERNEL_ELF)

# Flat binary
$(KERNEL_BIN): $(KERNEL_ELF)
	$(OBJCOPY) -O binary $(KERNEL_ELF) $(KERNEL_BIN)

# Образ диска
$(OS_IMG): $(BOOT_BIN) $(KERNEL_BIN)
	cat $(BOOT_BIN) $(KERNEL_BIN) > $(OS_IMG)
	python3 -c "\
import os; \
f=open('$(OS_IMG)','ab'); \
s=os.path.getsize('$(OS_IMG)'); \
f.write(b'\x00'*(1474560-s) if s<1474560 else b''); \
f.close(); \
print('Image:', os.path.getsize('$(OS_IMG)'), 'bytes')"

# Запуск
run: $(OS_IMG)
	$(QEMU) -drive format=raw,file=$(OS_IMG) -m 32M

# Отладка
debug: $(OS_IMG)
	$(QEMU) -drive format=raw,file=$(OS_IMG) -m 32M -s -S &
	gdb -ex "target remote :1234" \
	    -ex "set architecture i386" \
	    -ex "symbol-file $(KERNEL_ELF)"

clean:
	rm -rf target
	cd kernel && $(CARGO) clean
