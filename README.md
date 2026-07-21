# nexec-lgc

A lightweight legacy BIOS boot manager for x86 Linux and Windows.
Finds your installed operating systems, presents a boot menu, and loads the kernel.

This is the BIOS/legacy counterpart of
[nexec](https://github.com/person134/nexec), which targets UEFI systems.

## Installation

```bash
curl -LO https://github.com/person134/nexec-lgc/releases/latest/download/nexec-lgc
chmod +x nexec-lgc
sudo ./nexec-lgc install
```

This writes the bootloader to the MBR of your boot disk and copies itself
to `/usr/bin/nexec-lgc`. Reboot and nexec-lgc will appear as your boot menu.

To install manually (or on a different disk):

```bash
curl -LO https://github.com/person134/nexec-lgc/releases/latest/download/nexec-lgc-bios
sudo dd if=nexec-lgc-bios of=/dev/sdX bs=512 count=1 conv=notrunc
```

Replace `/dev/sdX` with your boot disk (e.g. `/dev/sda`).

To build from source instead, see [nexec-lgc on GitHub](https://github.com/person134/nexec-lgc).
