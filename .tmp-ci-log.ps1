$ErrorActionPreference = 'SilentlyContinue'
$e = Get-WinEvent -LogName 'Microsoft-Windows-CodeIntegrity/Operational' -MaxEvents 60 | Where-Object { $_.Id -eq 3118 } | Select-Object -First 1
"=== 3118 XML ==="
$e.ToXml()
""
$e2 = Get-WinEvent -LogName 'Microsoft-Windows-CodeIntegrity/Operational' -MaxEvents 60 | Where-Object { $_.Id -eq 3077 -and $_.Message -match 'target' } | Select-Object -First 1
"=== 3077 target XML ==="
$e2.ToXml()
""
"=== all blocked paths (3033, last 500 events) ==="
Get-WinEvent -LogName 'Microsoft-Windows-CodeIntegrity/Operational' -MaxEvents 500 | Where-Object Id -eq 3033 | ForEach-Object { if ($_.Message -match 'attempted to load (\\Device\S+) that') { $matches[1] } } | Sort-Object -Unique
