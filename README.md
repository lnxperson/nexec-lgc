# nexec-lgc

A lightweight legacy BIOS boot manager for x86 Linux and Windows.
Finds your installed operating systems, presents a boot menu, and loads the kernel.

This is the BIOS/legacy counterpart of
[nexec](https://github.com/person134/nexec), which targets UEFI systems.

## Installation

### Quick install

```bash
curl -LO https://github.com/person134/nexec-lgc/releases/latest/download/nexec-lgc
chmod +x nexec-lgc
sudo ./nexec-lgc install
```

The installer writes the bootloader to the MBR of your boot disk and copies
itself to `/usr/bin/nexec-lgc`. On next boot, GRUB (or your boot manager of
choice) will show **nexec-lgc** as a boot option if you chainload it —
alternatively, set nexec-lgc as the default MBR bootloader.

### Manual install (write MBR directly)

```bash
curl -LO https://github.com/person134/nexec-lgc/releases/latest/download/nexec-lgc-bios
sudo dd if=nexec-lgc-bios of=/dev/sda bs=512 count=1 conv=notrunc
```

Replace `/dev/sda` with your boot disk.

### Build from source

```bash
cargo build --release -p nexec-lgc-bios
cargo build --release -p nexec-lgc
```

The bootloader binary: `target/x86_64-unknown-linux-gnu/release/nexec-lgc-bios`
The CLI tool: `target/release/nexec-lgc`

## How it works

nexec-lgc scans your boot partition for installed operating systems, generates
entries from config files in `\EFI\nexec\entries\*.conf`, and at boot shows a
menu of the detected entries.

| Detected | How |
|----------|------|
| Windows | `\bootmgr` on the active partition |
| Linux (kernel on partition) | `\vmlinuz-*` — kernel found on partition root, matching `initramfs-*.img` or `initrd.img-*` autodetected |
| UKI | `\EFI\Linux\*.efi` — unified kernel images |

## Commands

| Command | What it does |
|---------|--------------|
| `nexec-lgc install` | Build, copy to partition, write MBR |
| `nexec-lgc remove` | Restore original MBR and remove CLI |
| `nexec-lgc config edit` | Open config in `$EDITOR` |
| `nexec-lgc config detect` | Print detected entries as config |
| `nexec-lgc config init` | Generate a sample config file |
| `nexec-lgc config set-default <name>` | Set the auto-boot entry |
| `nexec-lgc detect` | List detected OSes on the boot partition |
| `nexec-lgc entry list` | List all boot entries with titles |
| `nexec-lgc entry add <name> --efi <path>` | Add a new boot entry |
| `nexec-lgc entry remove <name>` | Remove a boot entry |
| `nexec-lgc entry edit <name>` | Edit a boot entry in your editor |
| `nexec-lgc entry mark-good <name>` | Mark entry as good (remove boot counter) |
| `nexec-lgc entry set-tries <name> <N>` | Set boot tries for an entry |
| `nexec-lgc update` | Pull latest release and reinstall |

### Flags

| Flag | Commands | Purpose |
|------|----------|---------|
| `--disk /dev/sda` | install | Specify target disk for MBR install |
| `--efi /path/to/binary --no-build` | install | Skip rebuild, use prebuilt binary |

## Configuration

Boot entries are stored as individual `.conf` files in `\EFI\nexec\entries\`.
The main configuration at `\EFI\nexec\nexec.conf` holds global settings only.
Generated automatically by `nexec-lgc install`. Edit with `nexec-lgc config edit`:

### Example Main config (`\EFI\nexec\nexec.conf`)

```ini
default = arch
timeout = 5
order = arch windows
# no_scan = true    # uncomment to skip auto-detection

# Keybinds — change these for non-QWERTY keyboard layouts
key_manual = m
key_reboot = r
key_shutdown = s
key_backup = b
```

### Example Linux Entry file (e.g. `\EFI\nexec\entries\arch.conf`)

```ini
title = Arch Linux
efi = \vmlinuz-linux
options = root=UUID=your-uuid rw quiet
initrd = \initramfs-linux.img
```

### Entry with boot counter (`\EFI\nexec\entries\arch+3.conf`)

A `+N` suffix in the filename sets the boot counter. The entry will be
auto-selected at most N times. After each boot the counter decrements;
when exhausted the entry is hidden. Mark it good via userspace once the
system comes up:

```bash
sudo nexec-lgc entry mark-good arch
```

An entry like `arch+3.conf` becomes `arch.conf`.

### Windows Entry file (`\EFI\nexec\entries\windows.conf`)

```ini
title = Windows
efi = \bootmgr
```

| Key | Where | Description |
|-----|-------|-------------|
| `default` | `nexec.conf` | Entry auto-selected when timeout expires |
| `timeout` | `nexec.conf` | Seconds before auto-boot (0 = wait forever) |
| `order` | `nexec.conf` | Space-separated display order |
| `no_scan` | `nexec.conf` | Use only config entries (skip auto-detect) |
| `key_manual` | `nexec.conf` | Key for manual boot browser (default: `m`) |
| `key_reboot` | `nexec.conf` | Key to reboot (default: `r`) |
| `key_shutdown` | `nexec.conf` | Key to shutdown (default: `s`) |
| `key_backup` | `nexec.conf` | Key to restore backup entries (default: `b`) |
| `title` | `entries/*.conf` | Display name in the menu |
| `efi` | `entries/*.conf` | Path to the kernel/bootloader on the partition |
| `options` | `entries/*.conf` | Kernel command-line arguments |
| `initrd` | `entries/*.conf` | Path to initramfs on the partition |
| `+N` suffix | filename | Boot counter — decrements each boot, entry hidden at 0 |

## Boot counting

nexec-lgc supports automatic fallback with boot counters.
Name an entry file `name+N.conf` where N is the number
of allowed boot attempts:

- `arch+3.conf` — allows 3 boot attempts
- Each time nexec-lgc boots it, the counter decrements (`+3` → `+2` → `+1`)
- On the last try (`+1` → no suffix), the entry becomes a normal entry
- If booting fails repeatedly and the counter reaches 0, the entry is
  hidden from the menu and the next entry in `order` is tried
- After a successful boot, run `nexec-lgc entry mark-good arch` to remove
  the counter (renames `arch+3.conf` → `arch.conf`)

Set up boot counting for a kernel update:

```bash
sudo nexec-lgc entry set-tries linux-testing 3
```

This renames `linux-testing.conf` to `linux-testing+3.conf`.

## Boot menu

Entries are centered in a boxdrawn UI. Titles are centered within
each entry line. The selected entry is highlighted.

Boot counters show `[N]` next to the name (remaining tries).

| Key | Action |
|-----|--------|
| `↑`/`↓` | Select entry |
| `1`–`9` | Direct entry selection |
| `Enter` | Boot selected entry |
| configurable (`m` by default) | Browse all `.efi` files on the partition |
| configurable (`r` by default) | Reboot |
| configurable (`s` by default) | Shutdown |
| configurable (`b` by default) | Restore backup entries |

If the boot fails, a recovery menu offers reboot, restore backup entries,
file browser, or shutdown.

## One-key recovery

Before every boot and before any `nexec-lgc entry` command, nexec-lgc backs up
`\EFI\nexec\entries\*.conf` to `\EFI\nexec\backup\entries\*.conf`.

At startup, hold **r** for 2 seconds to restore the last backup:

```
Hold r for recovery...
```

This overwrites the current entries with the backed-up versions. Use it
when a bad entry or boot counter change leaves you unable to boot.

You can also restore backups from the recovery menu (option **b**) after a
failed boot.

## One-time setup

If your kernel is not on the boot partition, copy it there once:

```bash
# Find your kernel version
KVER=$(uname -r)
sudo mount /dev/sda1 /mnt   # mount your boot partition
sudo cp /boot/vmlinuz-$KVER /boot/initramfs-$KVER.img /mnt/
sudo nexec-lgc install
```

nexec-lgc auto-detects any `vmlinuz-*` file on the boot partition, regardless
of distribution.

## License

MIT
