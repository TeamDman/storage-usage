# Storage Usage v2

A Rust-based command-line tool for analyzing Windows NTFS Master File Table (MFT) data and performing low-level storage operations.

## Features

- **Complete MFT Dumping**: Extract the entire Master File Table from NTFS volumes, properly handling fragmentation
- **MFT Analysis**: Query, summarize, and compare MFT files
- **Privilege Management**: Automatic elevation handling for administrative operations
- **NTFS Validation**: Verify filesystem compatibility before operations

## Installation

```bash
cargo build --release
```

## Usage

### MFT Operations

#### Dump MFT
Extract the complete Master File Table from an NTFS drive:

```bash
# Dump MFT from C: drive
storage-usage-v2 mft dump output.mft

# Dump from specific drive with overwrite
storage-usage-v2 mft dump output.mft --drive-letter D --overwrite-existing
```

**Features:**
- Automatically handles administrator privilege elevation
- Validates NTFS filesystem before dumping
- Parses boot sector and follows data runs for complete extraction
- Handles fragmented MFTs properly (unlike simple sector reading)
- Progress reporting with human-readable sizes

#### Query MFT
Search for specific files or file types within an MFT:

```bash
# Find all .exe files
storage-usage-v2 mft query mft_dump.bin "*.exe" --limit 50

# Find specific files (case-insensitive)
storage-usage-v2 mft query mft_dump.bin "notepad.exe" "*.dll" --ignore-case

# Show full paths
storage-usage-v2 mft query mft_dump.bin "*.log" --full-paths --limit 20
```

**Features:**
- Supports wildcard patterns (`*.ext`) and literal filenames
- Case-sensitive and case-insensitive matching
- Configurable result limits
- Option to show full paths or just filenames

#### Summarize MFT
Get statistical overview of an MFT file:

```bash
# Basic summary
storage-usage-v2 mft summarize mft_dump.bin

# Detailed statistics with sample paths
storage-usage-v2 mft summarize mft_dump.bin --verbose --show-paths

# Process only first 10000 entries (for large files)
storage-usage-v2 mft summarize mft_dump.bin --max-entries 10000
```

**Features:**
- Total file count and MFT size statistics
- File type distribution analysis
- Sample file paths for verification
- Performance-optimized for large MFT files

#### Compare MFTs
Compare two MFT files to find differences:

```bash
# Basic comparison
storage-usage-v2 mft diff old_mft.bin new_mft.bin

# Detailed byte-by-byte analysis
storage-usage-v2 mft diff old_mft.bin new_mft.bin --verbose --max-diffs 20
```

**Features:**
- High-level file count and size comparisons
- Detailed byte-level difference analysis
- Configurable diff output limits

### Elevation Management

#### Check Elevation Status
```bash
# Check if running with administrator privileges
storage-usage-v2 elevation check
```

#### Test Elevation
```bash
# Test elevation functionality
storage-usage-v2 elevation test
```

### Global Options

- `--debug`: Enable detailed debug logging
- `--help`: Show help information
- `--version`: Show version information

## Technical Details

### MFT Dumping Implementation

The tool implements proper NTFS data runs parsing to handle fragmented MFTs:

1. **Boot Sector Analysis**: Reads NTFS boot sector to get cluster size, MFT location
2. **MFT Record 0 Parsing**: Reads the MFT's own record to find its DATA attribute
3. **Data Runs Decoding**: Parses NTFS data runs to find all MFT fragments
4. **Sequential Reconstruction**: Reads each fragment and reconstructs complete MFT

This approach correctly handles MFTs that have grown beyond their initial reserved space and become fragmented across the disk.

### NTFS Validation

Before performing MFT operations, the tool validates:
- Drive is using NTFS filesystem
- Volume can be accessed with required privileges
- Boot sector contains valid NTFS parameters

### Privilege Handling

- Automatically detects when administrator privileges are required
- Relaunches with elevation when necessary
- Enables backup/restore privileges for system file access

## Examples

### Complete Workflow
```bash
# 1. Dump MFT from system drive
storage-usage-v2 mft dump system_mft.bin --drive-letter C

# 2. Get overview of the MFT
storage-usage-v2 mft summarize system_mft.bin --verbose

# 3. Find all executable files
storage-usage-v2 mft query system_mft.bin "*.exe" "*.dll" --full-paths --limit 100

# 4. Compare with previous dump
storage-usage-v2 mft diff old_system_mft.bin system_mft.bin
```

### Forensic Analysis
```bash
# Dump MFT for forensic analysis
storage-usage-v2 mft dump evidence_mft.bin --drive-letter E --overwrite-existing

# Find specific file types of interest
storage-usage-v2 mft query evidence_mft.bin "*.doc" "*.pdf" "*.jpg" --ignore-case --full-paths

# Get comprehensive statistics
storage-usage-v2 mft summarize evidence_mft.bin --verbose --show-paths
```

## Requirements

- Windows operating system
- NTFS filesystem for MFT operations
- Administrator privileges for MFT dumping
- Rust 1.70+ for building from source

## Links

For parsing discontiguous MFT

https://github.com/pitest3141592653/sysMFT/blob/e0b60a040ccdd07337a9715777e455a82f64b216/main.py


https://www.futurelearn.com/info/courses/introduction-to-malware-investigations/0/steps/146529

https://learn.microsoft.com/en-us/windows/win32/fileio/master-file-table
https://learn.microsoft.com/en-us/windows/win32/devnotes/master-file-table
https://learn.microsoft.com/en-us/troubleshoot/windows-server/backup-and-storage/ntfs-reserves-space-for-mft
https://github.com/libyal/libfsntfs/blob/82181db7c9f272f98257cf3576243d9ccbbe8823/documentation/New%20Technologies%20File%20System%20(NTFS).asciidoc