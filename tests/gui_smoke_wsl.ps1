# Runs tests/gui_smoke_wsl.sh inside the WSL distro and keeps the "Finder" window
# in the foreground on Windows while it runs. WSLg only delivers keyboard input to the window
# Windows considers active, so without this the typed keys are silently dropped.
#
# It takes keyboard focus for about a minute: do not type while it runs.
# Usage (from the repo on Windows):  pwsh tests/gui_smoke_wsl.ps1 [-Distro archlinux] [-LinuxRepo /root/labyrinth]
param(
  [string]$Distro = "archlinux",
  [string]$LinuxRepo = "/root/labyrinth",
  [string]$Binary = "./target/debug/finder"
)
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public class FG {
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
function Focus-LabyrinthWindow {
  [void][FG]::EnumWindows({ param($h, $l)
    $sb = New-Object Text.StringBuilder 256
    [void][FG]::GetWindowText($h, $sb, 256)
    if ($sb.ToString() -like "*Finder*" -and [FG]::IsWindowVisible($h)) { [void][FG]::SetForegroundWindow($h) }
    $true }, [IntPtr]::Zero)
}

$job = Start-Job -ScriptBlock {
  param($d, $repo, $bin)
  wsl -d $d -u root -- bash -c "cd $repo/packages/finder && bash ../../tests/gui_smoke_wsl.sh $bin" 2>&1
} -ArgumentList $Distro, $LinuxRepo, $Binary

while ($job.State -eq "Running") { Focus-LabyrinthWindow; Start-Sleep -Milliseconds 700 }
$out = Receive-Job $job
$out
Remove-Job $job
if ($out -match "0 failed") { exit 0 } else { exit 1 }
