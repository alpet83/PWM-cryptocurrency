# find_git_owners.ps1
# Показывает все git.exe процессы с цепочкой родителей (до 3 уровней вверх).
# Запуск: powershell -File scripts\find_git_owners.ps1
# Для убийства сирот: powershell -File scripts\find_git_owners.ps1 -KillOrphans

param(
    [switch]$KillOrphans   # гасить git.exe у которых родитель мёртв
)

function Get-ProcInfo($pid) {
    try {
        $p = Get-Process -Id $pid -ErrorAction Stop
        return [PSCustomObject]@{
            PID  = $pid
            Name = $p.Name
            Dead = $false
        }
    } catch {
        return [PSCustomObject]@{
            PID  = $pid
            Name = "<dead>"
            Dead = $true
        }
    }
}

function Get-ParentChain($proc, $depth = 3) {
    $chain = @()
    $cur = $proc
    for ($i = 0; $i -lt $depth; $i++) {
        try {
            $wmi = Get-CimInstance Win32_Process -Filter "ProcessId=$($cur.Id)" -ErrorAction Stop
            $ppid = $wmi.ParentProcessId
            $info = Get-ProcInfo $ppid
            $chain += $info
            if ($info.Dead) { break }
            $cur = Get-Process -Id $ppid -ErrorAction Stop
        } catch { break }
    }
    return $chain
}

$gitProcs = Get-Process -Name "git" -ErrorAction SilentlyContinue
if (-not $gitProcs) {
    Write-Host "Нет активных git.exe процессов." -ForegroundColor Green
    exit 0
}

Write-Host ("=" * 90)
Write-Host ("  {0,-8}  {1,-50}  {2}" -f "git PID", "Цепочка родителей", "Статус")
Write-Host ("=" * 90)

$orphans = @()

foreach ($g in $gitProcs | Sort-Object Id) {
    $chain = Get-ParentChain $g
    $chainStr = ($chain | ForEach-Object {
        "$($_.PID):$($_.Name)"
    }) -join " <- "

    # Первый родитель мёртв → сирота
    $isOrphan = $chain.Count -gt 0 -and $chain[0].Dead
    $status = if ($isOrphan) { "ORPHAN" } else { "ok" }
    $color  = if ($isOrphan) { "Yellow" } else { "Cyan" }

    Write-Host ("  {0,-8}  {1,-50}  {2}" -f $g.Id, $chainStr, $status) -ForegroundColor $color

    if ($isOrphan) { $orphans += $g }
}

Write-Host ("=" * 90)
Write-Host "Итого git.exe: $($gitProcs.Count)  |  Сирот: $($orphans.Count)"

if ($KillOrphans -and $orphans.Count -gt 0) {
    Write-Host "`nГашу сирот..." -ForegroundColor Red
    foreach ($o in $orphans) {
        Write-Host "  Kill PID $($o.Id)" -ForegroundColor Red
        Stop-Process -Id $o.Id -Force -ErrorAction SilentlyContinue
    }
    Write-Host "Готово."
} elseif ($orphans.Count -gt 0) {
    Write-Host "`nЧтобы убить сирот: powershell -File scripts\find_git_owners.ps1 -KillOrphans" -ForegroundColor Yellow
}
