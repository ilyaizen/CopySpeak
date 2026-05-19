# Capture a screenshot of a running window by title (substring, case-insensitive).
# Usage: pwsh scripts/screenshot-window.ps1 [-WindowTitle "CopySpeak"] [-OutPath "static/screen-v0.1.4.png"] [-List]
param(
  [string]$WindowTitle = "CopySpeak",
  [string]$OutPath = "static/screen-v0.1.4.png",
  [switch]$List
)

Add-Type -AssemblyName System.Windows.Forms, System.Drawing

Add-Type @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class Win {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc enumProc, IntPtr lParam);
  [DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int dwAttribute, out RECT pvAttribute, int cbAttribute);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

  public static List<KeyValuePair<IntPtr, string>> Enumerate() {
    var list = new List<KeyValuePair<IntPtr, string>>();
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      int len = GetWindowTextLength(h);
      if (len == 0) return true;
      var sb = new StringBuilder(len + 1);
      GetWindowText(h, sb, sb.Capacity);
      list.Add(new KeyValuePair<IntPtr, string>(h, sb.ToString()));
      return true;
    }, IntPtr.Zero);
    return list;
  }
}
"@

$all = [Win]::Enumerate()

if ($List) {
  $all | ForEach-Object { "{0}  {1}" -f $_.Key, $_.Value } | Write-Host
  return
}

$matches = $all | Where-Object { $_.Value -like "*$WindowTitle*" }
if ($matches.Count -eq 0) {
  Write-Host "Visible windows (debug):"
  $all | ForEach-Object { Write-Host ("  '" + $_.Value + "'") }
  throw "No visible window title contains '$WindowTitle'."
}

$handle = $matches[0].Key
Write-Host ("Capturing window: '" + $matches[0].Value + "' (handle " + $handle + ")")

# SW_RESTORE = 9
[Win]::ShowWindow($handle, 9) | Out-Null
[Win]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 500

$rect = New-Object Win+RECT
$DWMWA_EXTENDED_FRAME_BOUNDS = 9
$hr = [Win]::DwmGetWindowAttribute($handle, $DWMWA_EXTENDED_FRAME_BOUNDS, [ref]$rect, [System.Runtime.InteropServices.Marshal]::SizeOf($rect))
if ($hr -ne 0) {
  [Win]::GetWindowRect($handle, [ref]$rect) | Out-Null
}

$width  = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { throw "Invalid window rect: ${width}x${height}" }

$bitmap   = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size $width, $height))

$resolvedDir = (Resolve-Path -LiteralPath (Split-Path -Parent $OutPath)).Path
$finalPath   = Join-Path $resolvedDir (Split-Path -Leaf $OutPath)
$bitmap.Save($finalPath, [System.Drawing.Imaging.ImageFormat]::Png)

$graphics.Dispose()
$bitmap.Dispose()

Write-Host "Saved $finalPath (${width} x ${height})"
