# Patch pwm-data.json genesis_accounts[] to match current --genesis-file funding rows.
# Use when pwmd exits with: snapshot genesis mismatch: rows 2 != 4
# Uses Python (preserves valid JSON for pwmd; do not use ConvertTo-Json).

param(
    [string]$RepoRoot = (Split-Path -Parent $PSScriptRoot),
    [string]$GenesisFile = '',
    [string[]]$StateDirs = @(),
    [switch]$WhatIf
)

$ErrorActionPreference = 'Stop'
. (Join-Path $RepoRoot 'cy-cluster-common.ps1')

if (-not $GenesisFile) {
    $GenesisFile = $CyGenesis
}
if ($StateDirs.Count -eq 0) {
    $StateDirs = @($CyStateProposer, $CyStateAttester)
}

if (-not (Test-Path -LiteralPath $GenesisFile)) {
    Write-Error "Missing genesis: $GenesisFile"
}

$targets = @()
foreach ($dir in $StateDirs) {
    $pwm = Join-Path $dir 'pwm-data.json'
    if (Test-Path -LiteralPath $pwm) {
        $targets += $pwm
    }
    else {
        Write-Warning "Skip (no pwm-data.json): $pwm"
    }
}
if ($targets.Count -eq 0) {
    Write-Error 'No pwm-data.json files to patch'
}

$py = @"
import json, sys
from pathlib import Path
repo = Path(r'$RepoRoot')
genesis = json.loads(Path(r'$GenesisFile').read_text(encoding='utf-8'))
rows = [
    {'acct': a['acct_hex'], 'pubkey': a['pubkey_hex'], 'der_idx': a['der_idx']}
    for a in genesis['gen_cfg']['funding']['accounts']
]
what_if = $(if ($WhatIf) { 'True' } else { 'False' })
for rel in sys.argv[1:]:
    p = Path(rel)
    data = json.loads(p.read_text(encoding='utf-8'))
    old = len(data.get('genesis_accounts', []))
    if what_if:
        print(f'Would patch {p}: genesis_accounts {old} -> {len(rows)}')
        continue
    data['genesis_accounts'] = rows
    p.write_text(json.dumps(data, indent=2, ensure_ascii=False) + chr(10), encoding='utf-8')
    print(f'Patched {p}: genesis_accounts {old} -> {len(rows)}')
"@

$args = @('-c', $py) + $targets
& python @args
if ($LASTEXITCODE -ne 0) {
    Write-Error 'Python repair failed (is python on PATH?)'
}
