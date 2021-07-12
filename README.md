# woutan

## Installation

```
$ cargo install https://github.com/nickelc/woutan.git
```

## Usage

### List connected Samsung devices
```
$ woutan help detect
wuotan-detect 
list connected Samsung devices

USAGE:
    wuotan detect

FLAGS:
    -h, --help    Prints help information
```

#### Example
```
$ woutan detect
Bus 003 Device 014: ID 04e8:685d
```

### Print PIT from connected Samsung device
```
$ woutan help print-pit
wuotan-print-pit 
print the contents of the PIT from a connected device or a PIT file

USAGE:
    wuotan print-pit [OPTIONS]

FLAGS:
    -h, --help    Prints help information

OPTIONS:
    -f, --file <FILE>    read local PIT file
```

#### Example
```
$ woutan print-pit -f s3pit.dat
Entry Count: 16
Unknown 1: 1598902083
Unknown 2: 844251476
Unknown 3: 30797
Unknown 4: 0
Unknown 5: 0
Unknown 6: 0
Unknown 7: 0
Unknown 8: 0

--- Entry #0 ---
Binary Type: 0 (AP)
Device Type: 2 (MMC)
Identifier: 80
Attributes: 00000010 (Read-Only)
Update Attributes: 00000010 (FOTA)
Partition Block Size/Offset: 0
Partition Block Count: 1734
File Offset (Obsolete): 0
File Size (Obsolete): 0
Partition Name: BOOTLOADER
Flash Name: sboot.bin
FOTA Name:

--- Entry #1 ---
Binary Type: 0 (AP)
Device Type: 2 (MMC)
Identifier: 81
Attributes: 00000101 (Read/Write)
Update Attributes: 00000101 (FOTA)
Partition Block Size/Offset: 1734
Partition Block Count: 312
File Offset (Obsolete): 0
File Size (Obsolete): 0
Partition Name: TZSW
Flash Name: tz.img
FOTA Name:

--- Entry #2 ---
Binary Type: 0 (AP)
Device Type: 2 (MMC)
Identifier: 70
Attributes: 00000101 (Read/Write)
Update Attributes: 00000101 (FOTA)
Partition Block Size/Offset: 34
Partition Block Count: 16
File Offset (Obsolete): 0
File Size (Obsolete): 0
Partition Name: PIT
Flash Name: mx.pit
FOTA Name:

...
```
