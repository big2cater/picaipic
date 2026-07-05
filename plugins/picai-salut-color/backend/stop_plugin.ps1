$ErrorActionPreference = 'SilentlyContinue'

$pluginMain = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot 'main.py'))
$port = 8011
if ($env:PICAIPIC_PLUGIN_PORT) {
  $parsedPort = 0
  if ([int]::TryParse($env:PICAIPIC_PLUGIN_PORT, [ref]$parsedPort)) {
    $port = $parsedPort
  }
}

Get-CimInstance Win32_Process -Filter "name='python.exe' or name='pythonw.exe'" |
  Where-Object { $_.CommandLine -and $_.CommandLine -like ('*' + $pluginMain + '*') } |
  ForEach-Object { & taskkill.exe /PID $_.ProcessId /T /F | Out-Null }

$pids = netstat.exe -ano -p tcp |
  ForEach-Object {
    $columns = ($_ -split '\s+') | Where-Object { $_ }
    if ($columns.Count -ge 5 -and $columns[0] -eq 'TCP' -and $columns[1] -like "*:$port" -and $columns[3] -eq 'LISTENING') {
      $columns[4]
    }
  } |
  Sort-Object -Unique

foreach ($pidValue in $pids) {
  & taskkill.exe /PID $pidValue /T /F | Out-Null
}
