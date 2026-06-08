#!/usr/bin/env python3
"""
dRun — тулчейн сборки PINDOS / Hammam kernel.

Использование:
  python drun.py -b              # собрать debug
  python drun.py -c              # cargo check всех компонентов
  python drun.py -T              # собрать + запустить QEMU с отладкой
  python drun.py -r 0.1-moorino  # собрать release ISO для реального железа
"""

import argparse
import subprocess
import shutil
import sys
import os
from pathlib import Path
from datetime import datetime

# ── Пути ──────────────────────────────────────────────────────────────────────
ROOT        = Path(__file__).parent.resolve()
DRUNNED     = ROOT / "drunned"
HAMMAM_DIR  = ROOT / "hammam"
TOOLS_DIR   = ROOT / "tools"

KERNEL_DEBUG   = HAMMAM_DIR / "target/x86_64-unknown-none/debug/hammam-kernel"
KERNEL_RELEASE = HAMMAM_DIR / "target/x86_64-unknown-none/release/hammam-kernel"
ISO_PATH       = ROOT / "hammam.iso"

TARGET = "x86_64-unknown-none"

# ── Утилиты ───────────────────────────────────────────────────────────────────
def banner(text: str):
    line = "=" * 60
    print(f"\n{line}")
    print(f"  {text}")
    print(f"{line}\n")

def run(cmd: list[str], cwd: Path = ROOT, check: bool = True) -> int:
    print(f"  $ {' '.join(str(c) for c in cmd)}")
    result = subprocess.run(cmd, cwd=cwd, check=False)
    if check and result.returncode != 0:
        print(f"\n[FAIL] команда завершилась с кодом {result.returncode}")
        sys.exit(result.returncode)
    return result.returncode

def wsl(cmd: str, check: bool = True) -> int:
    """Выполнить команду внутри WSL2."""
    # Просто выполняем команду в WSL, предполагая что мы уже в правильной директории
    # WSL автоматически монтирует C: в /mnt/c
    full_cmd = cmd
    return run(["wsl", "bash", "-c", full_cmd], check=check)

def ensure_drunned():
    DRUNNED.mkdir(exist_ok=True)

def copy_artifact(src: Path, dest_name: str):
    ensure_drunned()
    dest = DRUNNED / dest_name
    shutil.copy2(src, dest)
    size = dest.stat().st_size / 1024
    print(f"  → drunned/{dest_name} ({size:.1f} KiB)")

# ── Команды ───────────────────────────────────────────────────────────────────
def cmd_check():
    """-c : cargo check всех компонентов."""
    banner("dRun CHECK")
    
    components = [
        (HAMMAM_DIR, "Hammam kernel", True),  # True = use x86_64-unknown-none target
    ]
    
    # Добавить userspace компоненты  
    # net-server требует Linux для сборки (используется x86_64-unknown-linux-gnu)
    for name in ["userspace/hello"]:
        path = ROOT / name
        if (path / "Cargo.toml").exists():
            components.append((path, name, True))
    
    all_ok = True
    for path, label, use_custom_target in components:
        print(f"  Checking {label}...")
        
        if use_custom_target:
            args = ["cargo", "check", "--target", TARGET]
        else:
            args = ["cargo", "check"]
        
        result = subprocess.run(
            args,
            cwd=path, 
            check=False,
            capture_output=False
        )
        
        if result.returncode != 0:
            print(f"  [FAIL] {label}")
            all_ok = False
        else:
            print(f"  [OK]   {label}")
    
    print(f"\n  Note: userspace/net-server skipped (requires Linux host)")
    
    if all_ok:
        print("\n[OK] Все компоненты прошли проверку.")
    else:
        print("\n[FAIL] Есть ошибки — см. выше.")
        sys.exit(1)

def cmd_build():
    """-b : собрать debug сборку."""
    banner("dRun BUILD (debug)")
    
    print("  Сборка Hammam kernel...")
    run(["cargo", "build", "--target", TARGET], cwd=HAMMAM_DIR)
    
    print("\n  Создание ISO через WSL2...")
    wsl("bash tools/make_iso.sh")
    
    ensure_drunned()
    copy_artifact(KERNEL_DEBUG, "hammam-kernel-debug")
    copy_artifact(ISO_PATH, "pindos-debug.iso")
    
    print("\n[OK] Debug сборка готова → drunned/")

def cmd_test():
    """-T : собрать + запустить QEMU с отладочным выводом."""
    banner("dRun TEST (QEMU debug)")
    
    # Сначала собрать
    print("  Сборка Hammam kernel...")
    run(["cargo", "build", "--target", TARGET], cwd=HAMMAM_DIR)
    
    print("  Создание ISO...")
    wsl("bash tools/make_iso.sh")
    copy_artifact(ISO_PATH, "pindos-debug.iso")
    
    # Запустить QEMU
    banner("QEMU — вывод ядра (COM1)")
    print("  Для остановки: Ctrl+C\n")
    
    qemu_cmd = [
        "qemu-system-x86_64",
        "-M", "pc",
        "-cdrom", str(ISO_PATH),
        "-serial", "stdio",
        "-display", "none",
        "-m", "256M",
        "-netdev", "user,id=net0,hostfwd=tcp::8080-:80",
        "-device", "virtio-net-pci,netdev=net0",
        "-d", "int,cpu_reset",        # отладка: прерывания и CPU reset
        "-D", str(DRUNNED / "qemu-debug.log"),  # лог в файл
        "-no-reboot",                  # не перезагружаться при краше
    ]
    
    print(f"  $ {' '.join(qemu_cmd)}\n")
    print("─" * 60)
    
    try:
        subprocess.run(qemu_cmd, check=False)
    except KeyboardInterrupt:
        pass
    
    log = DRUNNED / "qemu-debug.log"
    if log.exists() and log.stat().st_size > 0:
        print(f"\n{'─' * 60}")
        print(f"  Отладочный лог QEMU → drunned/qemu-debug.log")
        print(f"  Последние 20 строк:")
        lines = log.read_text(errors="replace").splitlines()
        for line in lines[-20:]:
            print(f"    {line}")

def cmd_release(version: str):
    """−r <version> : release ISO для реального железа."""
    banner(f"dRun RELEASE v{version}")
    
    timestamp = datetime.now().strftime("%Y%m%d-%H%M")
    iso_name  = f"pindos-{version}-{timestamp}.iso"
    kernel_name = f"hammam-kernel-{version}"
    
    print("  Сборка Hammam kernel (release)...")
    run(["cargo", "build", "--target", TARGET, "--release"], cwd=HAMMAM_DIR)
    
    # Создать ISO из release бинаря
    print("  Создание release ISO через WSL2...")
    wsl(f"KERNEL_PATH=hammam/target/x86_64-unknown-none/release/hammam-kernel "
        f"bash tools/make_iso.sh")
    
    ensure_drunned()
    copy_artifact(KERNEL_RELEASE, kernel_name)
    copy_artifact(ISO_PATH, iso_name)
    
    # Записать метаданные релиза
    meta = DRUNNED / f"pindos-{version}-{timestamp}.txt"
    meta.write_text(
        f"PINDOS Release\n"
        f"Version:   {version}\n"
        f"Built:     {datetime.now().isoformat()}\n"
        f"Kernel:    {kernel_name}\n"
        f"ISO:       {iso_name}\n"
        f"Target:    {TARGET}\n"
    )
    
    print(f"\n[OK] Release готов:")
    print(f"     drunned/{iso_name}")
    print(f"     drunned/{kernel_name}")
    print(f"     drunned/{meta.name}")
    print(f"\n  Запись на флешку (пример):")
    print(f"     dd if=drunned/{iso_name} of=/dev/sdX bs=4M status=progress")

def cmd_clean():
    """-cl : очистить все артефакты сборки."""
    banner("dRun CLEAN")
    
    items_to_clean = []
    
    # Cargo target директории
    cargo_projects = [
        HAMMAM_DIR,
        ROOT / "userspace" / "hello",
        ROOT / "userspace" / "net-server",
        ROOT / "userspace" / "nvme-driver",
        ROOT / "userspace" / "dealduck",
        ROOT / "userspace" / "posix-compat",
    ]
    
    for project in cargo_projects:
        target_dir = project / "target"
        if target_dir.exists():
            items_to_clean.append((target_dir, f"{project.name}/target"))
    
    # ISO и временные файлы
    if ISO_PATH.exists():
        items_to_clean.append((ISO_PATH, "hammam.iso"))
    
    iso_new = ROOT / "hammam.iso.new"
    if iso_new.exists():
        items_to_clean.append((iso_new, "hammam.iso.new"))
    
    iso_root = ROOT / "iso_root"
    if iso_root.exists():
        items_to_clean.append((iso_root, "iso_root/"))
    
    # Логи
    logs = [
        ROOT / "serial.log",
        ROOT / "qemu_debug.log",
    ]
    for log in logs:
        if log.exists():
            items_to_clean.append((log, log.name))
    
    # Артефакты drunned (опционально - спрашиваем)
    if DRUNNED.exists():
        print("  Папка drunned/ содержит собранные артефакты.")
        print("  Очистить её? (y/N): ", end="", flush=True)
        response = input().strip().lower()
        if response in ['y', 'yes', 'д', 'да']:
            items_to_clean.append((DRUNNED, "drunned/"))
    
    # Разное
    misc = [
        ROOT / "liblib_simple.rlib",
    ]
    for item in misc:
        if item.exists():
            items_to_clean.append((item, item.name))
    
    if not items_to_clean:
        print("  Нечего чистить - репозиторий уже чистый!")
        return
    
    print(f"  Найдено {len(items_to_clean)} элементов для удаления:\n")
    
    total_size = 0
    for path, label in items_to_clean:
        if path.is_dir():
            size = sum(f.stat().st_size for f in path.rglob('*') if f.is_file())
        else:
            size = path.stat().st_size
        total_size += size
        size_mb = size / (1024 * 1024)
        print(f"    • {label:<40} ({size_mb:>8.2f} MB)")
    
    print(f"\n  Общий размер: {total_size / (1024 * 1024):.2f} MB")
    print(f"  Удалить всё? (y/N): ", end="", flush=True)
    
    response = input().strip().lower()
    if response not in ['y', 'yes', 'д', 'да']:
        print("\n  [ОТМЕНЕНО] Очистка отменена.")
        return
    
    print("\n  Удаление...")
    removed = 0
    for path, label in items_to_clean:
        try:
            if path.is_dir():
                shutil.rmtree(path)
                print(f"    ✓ {label}")
            else:
                path.unlink()
                print(f"    ✓ {label}")
            removed += 1
        except Exception as e:
            print(f"    ✗ {label} - {e}")
    
    print(f"\n[OK] Удалено {removed}/{len(items_to_clean)} элементов.")
    print(f"[OK] Освобождено ~{total_size / (1024 * 1024):.2f} MB.")
    print(f"\n  Директория очищена!")


# ── Точка входа ───────────────────────────────────────────────────────────────
def main():
    parser = argparse.ArgumentParser(
        prog="drun",
        description="dRun — тулчейн сборки PINDOS"
    )
    parser.add_argument("-b", action="store_true", help="собрать debug сборку")
    parser.add_argument("-c", action="store_true", help="проверить код на ошибки")
    parser.add_argument("-T", action="store_true", help="собрать и запустить QEMU")
    parser.add_argument("-r", metavar="VERSION",   help="release ISO (например: 0.1-moorino)")
    parser.add_argument("-cl", "--clean", action="store_true", help="очистить артефакты сборки")
    
    args = parser.parse_args()
    
    if not any([args.b, args.c, args.T, args.r, args.clean]):
        parser.print_help()
        sys.exit(0)
    
    if args.c:
        cmd_check()
    elif args.b:
        cmd_build()
    elif args.T:
        cmd_test()
    elif args.r:
        cmd_release(args.r)
    elif args.clean:
        cmd_clean()

if __name__ == "__main__":
    main()
