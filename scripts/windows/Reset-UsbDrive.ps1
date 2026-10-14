<#
.SYNOPSIS
  Wipe and reformat a USB drive from Windows, including one Windows reports as "protected" / read-only.

.DESCRIPTION
  Does what a reboot into Linux + cfdisk does, without the reboot:
    1. lists only USB disks (never the disk Windows runs from, never an internal disk);
    2. clears the software write-protection that makes Windows, Rufus and diskpart refuse:
         - the disk's read-only flag and the offline flag,
         - the system-wide StorageDevicePolicies\WriteProtect registry switch,
         - leftover BitLocker / old partition tables (by wiping the partition table);
    3. wipes the partition table (and with -Full, every sector), makes a new one, formats it.
  EVERYTHING on the chosen drive is lost.

  What it cannot do: a drive with a physical write-protect switch (slide it), or a flash drive whose
  controller has locked itself read-only because the flash is worn out (cheap drives do this when they
  start to fail). The script tells you which case it is if the drive stays read-only after the steps.

.PARAMETER Fs
  exfat (default), fat32, ntfs, or none (leave it empty and unformatted, for Rufus or dd afterwards).

.PARAMETER Style
  gpt (default) or mbr (for old BIOS-only machines).

.PARAMETER Label
  Volume label (default USB).

.PARAMETER Full
  Overwrite the whole drive with zeros first (slow: minutes to hours; a quick wipe is usually enough).

.PARAMETER List
  Only list the USB disks and exit (works without admin).

.EXAMPLE
  .\Reset-UsbDrive.ps1
  .\Reset-UsbDrive.ps1 -Fs none          # blank the drive, then write the ISO with Rufus
  .\Reset-UsbDrive.ps1 -Fs fat32 -Label LABYRINTH
#>
[CmdletBinding()]
param(
    [ValidateSet('exfat', 'fat32', 'ntfs', 'none')][string]$Fs = 'exfat',
    [ValidateSet('gpt', 'mbr')][string]$Style = 'gpt',
    [string]$Label = 'USB',
    [switch]$Full,
    [switch]$List
)

$ErrorActionPreference = 'Stop'

function Test-Admin {
    ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-UsbDisks {
    # BusType USB only, and never the boot/system disk. Some card readers and USB-SATA bridges report
    # as SD or SATA; those are left out on purpose rather than risk an internal disk.
    Get-Disk | Where-Object { $_.BusType -eq 'USB' -and -not $_.IsBoot -and -not $_.IsSystem }
}

function Show-Disks($disks) {
    $disks | ForEach-Object {
        $letters = (Get-Partition -DiskNumber $_.Number -ErrorAction SilentlyContinue |
            Where-Object DriveLetter | ForEach-Object { "$($_.DriveLetter):" }) -join ' '
        [pscustomobject]@{
            Disk       = $_.Number
            Name       = $_.FriendlyName
            'Size GB'  = [math]::Round($_.Size / 1GB, 1)
            Style      = $_.PartitionStyle
            ReadOnly   = $_.IsReadOnly
            Offline    = $_.IsOffline
            Letters    = $letters
        }
    } | Format-Table -AutoSize | Out-String | Write-Host
}

if (-not (Test-Admin)) {
    if ($List) { Show-Disks (Get-UsbDisks); exit }
    # relaunch elevated with the same arguments
    $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-Fs', $Fs, '-Style', $Style, '-Label', "`"$Label`"")
    if ($Full) { $argList += '-Full' }
    Start-Process powershell -Verb RunAs -ArgumentList $argList
    exit
}

$disks = @(Get-UsbDisks)
if ($List) { Show-Disks $disks; exit }
if (-not $disks) {
    Write-Host 'No USB disk found. Plug the drive in, then run this again.' -ForegroundColor Yellow
    Read-Host 'Press Enter to close'; exit 1
}

Write-Host "`nUSB disks (the system disk and internal disks are never listed):" -ForegroundColor Cyan
Show-Disks $disks
$n = Read-Host 'Disk number to ERASE'
$d = $disks | Where-Object { "$($_.Number)" -eq $n.Trim() }
if (-not $d) { Write-Host 'That is not one of the USB disks above. Nothing was changed.' -ForegroundColor Red; Read-Host 'Press Enter to close'; exit 1 }

$gb = [math]::Round($d.Size / 1GB, 1)
Write-Host "`nAbout to erase disk $($d.Number): $($d.FriendlyName), $gb GB. Everything on it will be lost." -ForegroundColor Red
$ok = Read-Host "Type the size in GB ($gb) to confirm"
if ($ok.Trim() -ne "$gb") { Write-Host 'Not confirmed. Nothing was changed.'; Read-Host 'Press Enter to close'; exit 1 }

# 1. the system-wide write-protect switch some tools and policies set
$pol = 'HKLM:\SYSTEM\CurrentControlSet\Control\StorageDevicePolicies'
if ((Get-ItemProperty $pol -ErrorAction SilentlyContinue).WriteProtect) {
    Set-ItemProperty $pol -Name WriteProtect -Value 0
    Write-Host 'Cleared the WriteProtect registry policy (it was on).'
}

# 2. flags on the disk itself
Set-Disk -Number $d.Number -IsOffline $false -ErrorAction SilentlyContinue
Set-Disk -Number $d.Number -IsReadOnly $false -ErrorAction SilentlyContinue

# 3. let go of every volume on it, so nothing holds it open (Explorer, Rufus, an antivirus scan)
Get-Partition -DiskNumber $d.Number -ErrorAction SilentlyContinue | Where-Object DriveLetter | ForEach-Object {
    try { mountvol "$($_.DriveLetter):" /p | Out-Null } catch {}
}

# 4. diskpart does the part PowerShell's cmdlets refuse on a "protected" disk
$dp = New-Object System.Collections.Generic.List[string]
$dp.Add("select disk $($d.Number)")
$dp.Add('attributes disk clear readonly noerr')
$dp.Add('online disk noerr')
$dp.Add('clean')
if ($Full) { $dp.Add('clean all') }
$dp.Add("convert $Style")
if ($Fs -ne 'none') {
    $dp.Add('create partition primary')
    $dp.Add("format fs=$Fs label=`"$Label`" quick")
    $dp.Add('assign')
}
$script = Join-Path $env:TEMP 'reset-usb.diskpart'
Set-Content -Path $script -Value $dp -Encoding ASCII
Write-Host "`nWorking (diskpart)..." -ForegroundColor Cyan
$out = & diskpart /s $script 2>&1
Remove-Item $script -ErrorAction SilentlyContinue
$out | Write-Host

# 5. the result
Start-Sleep -Seconds 2
$after = Get-Disk -Number $d.Number
if ($after.IsReadOnly -or ($out -match 'write protected|media is write|The media is')) {
    Write-Host @"

The drive is STILL read-only after everything software can do. That means:
  - a physical write-protect switch on the drive or adapter (slide it the other way), or
  - the drive's own controller has locked it read-only. Cheap flash drives do this when the flash is
    worn out; it is a dying drive and cannot be fixed from any computer. Copy off what you need, then
    replace it. (Linux and cfdisk would hit the same wall.)
"@ -ForegroundColor Yellow
    Read-Host 'Press Enter to close'; exit 2
}
if ($out -match 'DiskPart has encountered an error|Virtual Disk Service error') {
    Write-Host "`nDiskpart reported an error (above). Unplug the drive, plug it into a different port (a direct one, not a hub), close Rufus and any Explorer window on it, and run this again." -ForegroundColor Yellow
    Read-Host 'Press Enter to close'; exit 3
}
Write-Host "`nDone. Disk $($d.Number) is now $($after.PartitionStyle)$(if ($Fs -ne 'none') { ", formatted $Fs, labelled $Label" } else { ', empty and unformatted' })." -ForegroundColor Green
Read-Host 'Press Enter to close'
