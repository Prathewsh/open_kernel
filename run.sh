#!/bin/bash
set -eo pipefail

# Define directories and outputs
BOOTLOADER_DIR="bootloader"
BUILD_DIR="build"
MNT_DIR="${BUILD_DIR}/mnt"

# Create build directory
mkdir -p "${BUILD_DIR}"

echo "🔨 Building the kernel..."
cargo build

echo "🔨 Building the UEFI bootloader..."
cd "${BOOTLOADER_DIR}"
cargo build --target x86_64-unknown-uefi
cd ..

echo "📁 Copying files to the disk directory..."
# Use QEMU's virtual FAT driver instead of building a raw image with mtools
# This ensures OVMF can read the files without needing a partition table.
rm -rf "${MNT_DIR}"
mkdir -p "${MNT_DIR}/EFI/BOOT"

# Copy the bootloader to the standard UEFI boot path
cp "${BOOTLOADER_DIR}/target/x86_64-unknown-uefi/debug/my_bootloader.efi" "${MNT_DIR}/EFI/BOOT/BOOTX64.EFI"

# Copy the kernel to the root of the virtual partition
cp target/x86_64-open_kernel/debug/open_kernel "${MNT_DIR}/kernel.elf"

echo "🚀 Running in QEMU..."
echo "⌨️  Type shell commands in this terminal or click the QEMU window."
echo "    Press Ctrl-C to stop QEMU."
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
    -drive format=raw,file=fat:rw:"${MNT_DIR}" \
    -serial stdio \
    -machine pc \
    -k en-us 2>&1 | tee serial.log
