@echo off
rem Double-click to wipe and reformat a USB drive (asks for admin). Options: Reset-UsbDrive.cmd -Fs none   (see Reset-UsbDrive.ps1)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Reset-UsbDrive.ps1" %*
