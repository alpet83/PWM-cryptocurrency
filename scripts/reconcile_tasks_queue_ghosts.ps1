# Reconcile stale tasks/queue (and tasks/in_progress) ghosts vs .cqds/team-tasks truth.
# UTF-8 with BOM for Windows PowerShell 5.1.
# Usage:
#   ./scripts/reconcile_tasks_queue_ghosts.ps1              # dry-run (default)
#   ./scripts/reconcile_tasks_queue_ghosts.ps1 -Apply       # move ghosts to tasks/_archive/queue-ghosts/

param(
    [switch]$Apply
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$TasksQueue = Join-Path $RepoRoot 'tasks\queue'
$TasksInProgress = Join-Path $RepoRoot 'tasks\in_progress'
$CqdsRoot = Join-Path $RepoRoot '.cqds\team-tasks'
$Archive = Join-Path $RepoRoot 'tasks\_archive\queue-ghosts'

function Get-TicketStatus {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    try {
        $j = Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
        return [string]$j.status
    }
    catch {
        return "parse_error"
    }
}

function Find-CqdsLocation {
    param([string]$Id)
    foreach ($sub in @('done', 'in_progress', 'queue', 'failed')) {
        $p = Join-Path $CqdsRoot "$sub\$Id.json"
        if (Test-Path -LiteralPath $p) {
            return @{ Sub = $sub; Path = $p; Status = (Get-TicketStatus $p) }
        }
    }
    return $null
}

$actions = @()

if (Test-Path -LiteralPath $TasksQueue) {
    Get-ChildItem -LiteralPath $TasksQueue -Filter '*.json' | ForEach-Object {
        $id = [System.IO.Path]::GetFileNameWithoutExtension($_.Name)
        $cq = Find-CqdsLocation $id
        $localStatus = Get-TicketStatus $_.FullName
        if ($cq -and $cq.Sub -eq 'done') {
            $actions += [pscustomobject]@{
                Action = 'archive_ghost_queue'
                Path   = $_.FullName
                Reason = "cqds done; local status=$localStatus"
            }
        }
        elseif ($cq -and $cq.Sub -eq 'in_progress') {
            $actions += [pscustomobject]@{
                Action = 'archive_ghost_queue'
                Path   = $_.FullName
                Reason = 'cqds in_progress; tasks/queue duplicate'
            }
        }
        elseif (-not $cq) {
            $actions += [pscustomobject]@{
                Action = 'keep_needs_share_ticket'
                Path   = $_.FullName
                Reason = 'not in .cqds/team-tasks — run share_ticket project_id=5'
            }
        }
    }
}

if (Test-Path -LiteralPath $TasksInProgress) {
    Get-ChildItem -LiteralPath $TasksInProgress -Filter '*.json' | ForEach-Object {
        $id = [System.IO.Path]::GetFileNameWithoutExtension($_.Name)
        $cq = Find-CqdsLocation $id
        $localStatus = Get-TicketStatus $_.FullName
        if ($cq -and $cq.Sub -eq 'done') {
            $actions += [pscustomobject]@{
                Action = 'archive_ghost_in_progress'
                Path   = $_.FullName
                Reason = "cqds done; local status=$localStatus"
            }
        }
        elseif ($localStatus -eq 'done' -and (Test-Path -LiteralPath (Join-Path $RepoRoot "tasks\done\$id.json"))) {
            $actions += [pscustomobject]@{
                Action = 'archive_ghost_in_progress'
                Path   = $_.FullName
                Reason = 'orchestrator tasks/done copy exists'
            }
        }
    }
}

Write-Host "=== reconcile_tasks_queue_ghosts ($([string]::Join('', $(if ($Apply) { 'APPLY' } else { 'DRY-RUN' })))) ==="
$actions | Format-Table -AutoSize

if (-not $Apply) {
    Write-Host 'No files changed. Re-run with -Apply to move ghosts to tasks/_archive/queue-ghosts/'
    exit 0
}

if (-not (Test-Path -LiteralPath $Archive)) {
    New-Item -ItemType Directory -Path $Archive -Force | Out-Null
}

foreach ($a in $actions) {
    if ($a.Action -notlike 'archive_*') { continue }
    $dest = Join-Path $Archive ([System.IO.Path]::GetFileName($a.Path))
    Move-Item -LiteralPath $a.Path -Destination $dest -Force
    Write-Host "Archived: $($a.Path) -> $dest"
}

Write-Host 'Done.'
