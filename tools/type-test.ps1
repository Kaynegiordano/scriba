# Test de bout en bout : tape du texte touche par touche (comme un vrai clavier)
# dans le Bloc-notes, éventuellement suivi de touches F1..F6, puis relit le contenu.
# Usage : powershell -File tools\type-test.ps1 -Text "la mezon{F1}est grande"
param([string]$Text = "la mezon{F1}", [int]$DelayMs = 40, [Int64]$Hwnd = 0, [switch]$NoRead, [switch]$NoClear)

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class Kb {
  [StructLayout(LayoutKind.Sequential)] struct KI { public ushort vk, scan; public uint flags, time; public IntPtr extra; }
  [StructLayout(LayoutKind.Explicit, Size=40)] struct IN { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KI ki; }
  [DllImport("user32.dll")] static extern uint SendInput(uint n, IN[] i, int size);
  [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint type);
  [DllImport("user32.dll")] public static extern short VkKeyScan(char c);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public static void Key(ushort vk, bool up) {
    var i = new IN[1]; i[0].type = 1; i[0].ki.vk = vk; i[0].ki.scan = (ushort)MapVirtualKey(vk, 0);
    i[0].ki.flags = up ? 2u : 0u; SendInput(1, i, 40);
  }
}
"@

if ($Hwnd -eq 0) {
  $Hwnd = (Get-Process Notepad -ErrorAction Stop | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle
}
$h = [IntPtr]$Hwnd
[Kb]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 300
if (-not $NoClear) {
  # tout effacer
  [Kb]::Key(0x11, $false); [Kb]::Key(0x41, $false); [Kb]::Key(0x41, $true); [Kb]::Key(0x11, $true); [Kb]::Key(0x2E, $false); [Kb]::Key(0x2E, $true)
  Start-Sleep -Milliseconds 200
}

$tokens = [regex]::Matches($Text, "\{F\d\}|\{\^\}|\{TOGGLE\}|.")
foreach ($t in $tokens) {
  $s = $t.Value
  if ($s -eq "{^}") {
    # touche morte ^ (AZERTY)
    $vk = [uint16]([Kb]::VkKeyScan([char]'^') -band 0xFF)
    [Kb]::Key($vk, $false); [Kb]::Key($vk, $true)
    Start-Sleep -Milliseconds $DelayMs
    continue
  }
  if ($s -eq "{TOGGLE}") {
    [Kb]::Key(0x11, $false); [Kb]::Key(0xA4, $false); [Kb]::Key(0x4C, $false); [Kb]::Key(0x4C, $true); [Kb]::Key(0xA4, $true); [Kb]::Key(0x11, $true)
    Start-Sleep -Milliseconds 200
    continue
  }
  if ($s -match "^\{F(\d)\}$") {
    $vk = [uint16](0x70 + [int]$Matches[1] - 1)
    [Kb]::Key($vk, $false); [Kb]::Key($vk, $true)
    Start-Sleep -Milliseconds 150
    continue
  }
  $r = [Kb]::VkKeyScan([char]$s)
  $vk = [uint16]($r -band 0xFF); $shift = ($r -band 0x100) -ne 0
  if ($shift) { [Kb]::Key(0x10, $false) }
  [Kb]::Key($vk, $false); [Kb]::Key($vk, $true)
  if ($shift) { [Kb]::Key(0x10, $true) }
  Start-Sleep -Milliseconds $DelayMs
}
Start-Sleep -Milliseconds 400
if ($NoRead) { return }

$root = [System.Windows.Automation.AutomationElement]::FromHandle($h)
$cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Document)
$doc = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
if (-not $doc) {
  $cond = New-Object System.Windows.Automation.PropertyCondition ([System.Windows.Automation.AutomationElement]::ControlTypeProperty), ([System.Windows.Automation.ControlType]::Edit)
  $doc = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}
$tp = $doc.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
"[" + $tp.DocumentRange.GetText(-1).TrimEnd("`r", "`n") + "]"
