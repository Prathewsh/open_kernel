#!/bin/bash
set -e

# Define directories and outputs
KERNEL_DIR="."
BOOTLOADER_DIR="bootloader"
BUILD_DIR="build"
IMG_FILE="${BUILD_DIR}/uefi_disk.img"
MNT_DIR="${BUILD_DIR}/mnt"

# Create build directory
mkdir -p "${BUILD_DIR}"

echo "🔨 Building the kernel..."
cargo build

echo "🔨 Building the UEFI bootloader..."
cd "${BOOTLOADER_DIR}"
cargo build --target x86_64-unknown-uefi
cd ..

echo "💿 Creating FAT32 disk image..."
# Create a 32MB raw image filled with zeros
dd if=/dev/zero of="${IMG_FILE}" bs=1M count=32

# Format it as FAT32
# mtools handles formatting raw files consistently across macOS and Linux
if ! command -v mformat >/dev/null 2>&1; then
    echo "❌ Error: mtools is required but not installed."
    echo "Please install it using your package manager:"
    echo "  macOS: brew install mtools"
    echo "  Linux: sudo apt install mtools"
    exit 1
fi
mformat -i "${IMG_FILE}" -F ::

echo "📁 Copying files to the disk image..."
# Use mtools to copy files without needing root/sudo to mount
# On macOS: brew install mtools
# On Linux: apt install mtools

# Create EFI/BOOT directory structure on the image
mmd -i "${IMG_FILE}" ::/EFI
mmd -i "${IMG_FILE}" ::/EFI/BOOT

# Copy the bootloader to the standard UEFI boot path
mcopy -i "${IMG_FILE}" "${BOOTLOADER_DIR}/target/x86_64-unknown-uefi/debug/my_bootloader.efi" ::/EFI/BOOT/BOOTX64.EFI

# Copy the kernel to the root of the partition
mcopy -i "${IMG_FILE}" target/x86_64-open_kernel/debug/open_kernel ::/kernel.elf

echo "🚀 Running in QEMU..."
# Note: You need the OVMF UEFI firmware. 
# macOS: brew install qemu (includes OVMF) -> /opt/homebrew/share/qemu/edk2-x86_64-code.fd
# Linux: apt install ovmf -> /usr/share/OVMF/OVMF_CODE.fd

OVMF_PATH=""
if [ -f "/opt/homebrew/share/qemu/edk2-x86_64-code.fd" ]; then
    OVMF_PATH="/opt/homebrew/share/qemu/edk2-x86_64-code.fd"
elif [ -f "/usr/share/OVMF/OVMF_CODE.fd" ]; then
    OVMF_PATH="/usr/share/OVMF/OVMF_CODE.fd"
else
    echo "⚠️ Warning: OVMF firmware not found. Please install OVMF or update the OVMF_PATH in this script."
    exit 1
fi

qemu-system-x86_64 \
    -drive if=pflash,format=raw,readonly=on,file="${OVMF_PATH}" \
    -drive format=raw,file="${IMG_FILE}" \
    -serial file:serial.log \
    -machine pc \
    -k en-us
