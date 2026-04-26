#!/bin/bash
# PINDOS — создание загрузочного USB
# Поддерживает: BIOS, CSM/UEFI (через GRUB), нативный UEFI (через GRUB EFI)
#
# Использование:
#   ./scripts/make_usb.sh /dev/sdX        # записать на USB
#   ./scripts/make_usb.sh --iso           # создать ISO образ
#   ./scripts/make_usb.sh --img           # создать гибридный IMG (BIOS + UEFI)

set -e

KERNEL_ELF="target/kernel.elf"
KERNEL_BIN="target/kernel.bin"
BOOT_BIN="target/boot.bin"
OUT_IMG="target/pindos_usb.img"
OUT_ISO="target/pindos.iso"

RED='\033[0;31m'
GRN='\033[0;32m'
YLW='\033[1;33m'
NC='\033[0m'

check_deps() {
    local missing=0
    for cmd in grub-mkrescue grub-install xorriso mformat mcopy; do
        if ! command -v $cmd &>/dev/null; then
            echo -e "${YLW}Warning: $cmd not found${NC}"
            missing=$((missing+1))
        fi
    done
    return $missing
}

# ── Режим 1: Прямая запись на USB (BIOS/CSM) ─────────────────────────────
write_bios_usb() {
    local DEV=$1
    echo -e "${GRN}Writing BIOS-compatible image to $DEV...${NC}"

    if [ ! -b "$DEV" ]; then
        echo -e "${RED}Error: $DEV is not a block device${NC}"
        exit 1
    fi

    # Предупреждение
    echo -e "${RED}WARNING: This will ERASE all data on $DEV${NC}"
    read -p "Continue? [y/N] " confirm
    [ "$confirm" != "y" ] && exit 0

    # Создаём образ 64MB
    dd if=/dev/zero of="$OUT_IMG" bs=1M count=64 status=progress

    # Записываем MBR загрузчик
    dd if="$BOOT_BIN" of="$OUT_IMG" conv=notrunc bs=512

    # Записываем ядро начиная с сектора 1
    dd if="$KERNEL_BIN" of="$OUT_IMG" conv=notrunc bs=512 seek=1

    # Записываем на USB
    sudo dd if="$OUT_IMG" of="$DEV" bs=4M status=progress
    sudo sync

    echo -e "${GRN}Done! Boot from $DEV on BIOS/CSM systems.${NC}"
}

# ── Режим 2: ISO образ с GRUB (BIOS + UEFI) ──────────────────────────────
make_iso() {
    echo -e "${GRN}Creating hybrid ISO (BIOS + UEFI via GRUB)...${NC}"

    # Создаём структуру ISO
    rm -rf /tmp/pindos_iso
    mkdir -p /tmp/pindos_iso/boot/grub
    mkdir -p /tmp/pindos_iso/EFI/BOOT

    # Копируем ядро
    cp "$KERNEL_ELF" /tmp/pindos_iso/boot/pindos.elf

    # GRUB конфиг
    cat > /tmp/pindos_iso/boot/grub/grub.cfg << 'EOF'
set timeout=3
set default=0

menuentry "PINDOS 0.1" {
    echo "Loading PINDOS kernel..."
    multiboot2 /boot/pindos.elf
    boot
}

menuentry "PINDOS 0.1 (serial debug)" {
    echo "Loading PINDOS kernel (debug)..."
    multiboot2 /boot/pindos.elf debug=serial
    boot
}
EOF

    # Создаём ISO с поддержкой BIOS и UEFI
    grub-mkrescue \
        --output="$OUT_ISO" \
        --compress=xz \
        /tmp/pindos_iso \
        2>/dev/null

    echo -e "${GRN}ISO created: $OUT_ISO${NC}"
    echo -e "  Size: $(du -h $OUT_ISO | cut -f1)"
    echo ""
    echo "To write to USB:"
    echo "  sudo dd if=$OUT_ISO of=/dev/sdX bs=4M status=progress"
    echo ""
    echo "Or use Ventoy — just copy the ISO to the Ventoy USB drive."
}

# ── Режим 3: Гибридный IMG (BIOS + UEFI через GRUB) ──────────────────────
make_hybrid_img() {
    echo -e "${GRN}Creating hybrid IMG (BIOS + UEFI)...${NC}"

    IMG_SIZE_MB=64
    IMG="$OUT_IMG"

    # Создаём пустой образ
    dd if=/dev/zero of="$IMG" bs=1M count=$IMG_SIZE_MB status=progress

    # Разметка: GPT с ESP разделом (для UEFI) и data разделом
    # Используем sgdisk если доступен, иначе fdisk
    if command -v sgdisk &>/dev/null; then
        sgdisk -Z "$IMG"
        sgdisk -n 1:2048:+32M  -t 1:EF00 -c 1:"EFI System"  "$IMG"
        sgdisk -n 2:0:0        -t 2:8300 -c 2:"PINDOS Data"  "$IMG"
    else
        echo -e "${YLW}sgdisk not found, using MBR layout${NC}"
        # MBR fallback — просто пишем загрузчик
        dd if="$BOOT_BIN" of="$IMG" conv=notrunc bs=512
        dd if="$KERNEL_BIN" of="$IMG" conv=notrunc bs=512 seek=1
        echo -e "${GRN}MBR image created: $IMG${NC}"
        return
    fi

    # Монтируем ESP раздел и устанавливаем GRUB EFI
    LOOP=$(sudo losetup -f --show -P "$IMG")
    sudo mkfs.fat -F32 "${LOOP}p1"

    mkdir -p /tmp/pindos_esp
    sudo mount "${LOOP}p1" /tmp/pindos_esp

    sudo mkdir -p /tmp/pindos_esp/EFI/BOOT
    sudo mkdir -p /tmp/pindos_esp/boot/grub

    # Копируем ядро на ESP
    sudo cp "$KERNEL_ELF" /tmp/pindos_esp/boot/pindos.elf

    # GRUB конфиг
    sudo tee /tmp/pindos_esp/boot/grub/grub.cfg > /dev/null << 'EOF'
set timeout=3
set default=0

menuentry "PINDOS 0.1" {
    multiboot2 /boot/pindos.elf
    boot
}
EOF

    # Устанавливаем GRUB EFI
    sudo grub-install \
        --target=x86_64-efi \
        --efi-directory=/tmp/pindos_esp \
        --boot-directory=/tmp/pindos_esp/boot \
        --removable \
        --no-nvram \
        2>/dev/null || true

    # Также устанавливаем GRUB BIOS (для CSM)
    sudo grub-install \
        --target=i386-pc \
        --boot-directory=/tmp/pindos_esp/boot \
        "$LOOP" \
        2>/dev/null || true

    sudo umount /tmp/pindos_esp
    sudo losetup -d "$LOOP"

    echo -e "${GRN}Hybrid image created: $IMG${NC}"
    echo "Supports: UEFI (x86_64), BIOS, CSM"
}

# ── Ventoy совет ──────────────────────────────────────────────────────────
ventoy_tip() {
    echo ""
    echo -e "${YLW}Tip: Easiest way to boot on modern hardware:${NC}"
    echo "  1. Install Ventoy on USB: https://ventoy.net"
    echo "  2. Copy $OUT_ISO to the USB drive"
    echo "  3. Boot from USB — select PINDOS from Ventoy menu"
    echo ""
    echo "Ventoy supports both BIOS and UEFI automatically."
}

# ── Главная логика ────────────────────────────────────────────────────────

# Проверяем что ядро собрано
if [ ! -f "$KERNEL_ELF" ] || [ ! -f "$BOOT_BIN" ]; then
    echo -e "${RED}Error: Build the kernel first: make${NC}"
    exit 1
fi

case "${1:-}" in
    --iso)
        make_iso
        ventoy_tip
        ;;
    --img)
        make_hybrid_img
        ventoy_tip
        ;;
    /dev/*)
        write_bios_usb "$1"
        ;;
    "")
        echo "Usage:"
        echo "  $0 /dev/sdX    — write BIOS image to USB"
        echo "  $0 --iso       — create hybrid ISO (BIOS + UEFI via GRUB)"
        echo "  $0 --img       — create hybrid IMG (BIOS + UEFI)"
        echo ""
        echo "For UEFI systems, use --iso and write with dd or Ventoy."
        ;;
    *)
        echo -e "${RED}Unknown option: $1${NC}"
        exit 1
        ;;
esac
