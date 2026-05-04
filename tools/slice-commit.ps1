[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [int]$SliceNumber,

    [string[]]$Files = @(),

    [string]$Scope = "sprint6",
    [string]$Verb = "record",
    [string]$Subject,
    [string]$Body,
    [switch]$IncludeStandardArtifacts,
    [switch]$FilesFromManifest,
    [string]$TaskArtifactPath = "tasks/20260424-sprint6-optimization.json",
    [string]$ManifestKeyPrefix = "review_evidence_manifest_slice",
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Add-UniqueFile {
    param(
        [System.Collections.Generic.List[string]]$Target,
        [string]$FilePath
    )

    if (-not [string]::IsNullOrWhiteSpace($FilePath) -and -not $Target.Contains($FilePath)) {
        $Target.Add($FilePath)
    }
}

function Resolve-ManifestFiles {
    param(
        [int]$Slice,
        [string]$ArtifactPath,
        [string]$KeyPrefix
    )

    if (-not (Test-Path -LiteralPath $ArtifactPath -PathType Leaf)) {
        throw "Task artifact file not found: $ArtifactPath"
    }

    $raw = Get-Content -LiteralPath $ArtifactPath -Raw
    $obj = $raw | ConvertFrom-Json
    $manifestKey = "$KeyPrefix$Slice"
    $manifest = $obj.PSObject.Properties[$manifestKey]
    if ($null -eq $manifest) {
        throw "Manifest key '$manifestKey' was not found in $ArtifactPath"
    }
    if ($null -eq $manifest.Value.allowed_files -or $manifest.Value.allowed_files.Count -eq 0) {
        throw "Manifest key '$manifestKey' does not contain allowed_files."
    }
    return [string[]]$manifest.Value.allowed_files
}

$selectedFiles = New-Object 'System.Collections.Generic.List[string]'
foreach ($file in $Files) {
    Add-UniqueFile -Target $selectedFiles -FilePath $file
}

if ($IncludeStandardArtifacts) {
    $standardArtifacts = @(
        "docs/reviews/sprint-6-checklist.md",
        "docs/reviews/sprint-6-review-report.md",
        "docs/reviews/sprint-6-status-note.md",
        "docs/reviews/sprint-6-test-report.md",
        "tasks/20260424-sprint6-optimization.json"
    )
    foreach ($artifact in $standardArtifacts) {
        Add-UniqueFile -Target $selectedFiles -FilePath $artifact
    }
}

if ($FilesFromManifest) {
    $manifestFiles = Resolve-ManifestFiles -Slice $SliceNumber -ArtifactPath $TaskArtifactPath -KeyPrefix $ManifestKeyPrefix
    foreach ($artifact in $manifestFiles) {
        Add-UniqueFile -Target $selectedFiles -FilePath $artifact
    }
}

if ($selectedFiles.Count -eq 0) {
    throw "No files selected. Use -Files and/or -IncludeStandardArtifacts and/or -FilesFromManifest."
}

$missingFiles = @()
foreach ($file in $selectedFiles) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
        $missingFiles += $file
    }
}

if ($missingFiles.Count -gt 0) {
    Write-Error "The following files do not exist:"
    foreach ($missing in $missingFiles) {
        Write-Error "  - $missing"
    }
    exit 1
}

Write-Host "Slice commit request:"
Write-Host "  SliceNumber: $SliceNumber"
Write-Host "  Scope: $Scope"
Write-Host "  Verb: $Verb"
Write-Host "  IncludeStandardArtifacts: $IncludeStandardArtifacts"
Write-Host "  FilesFromManifest: $FilesFromManifest"
if ($FilesFromManifest) {
    Write-Host "  TaskArtifactPath: $TaskArtifactPath"
    Write-Host "  ManifestKeyPrefix: $ManifestKeyPrefix"
}
Write-Host "  Files:"
foreach ($file in $selectedFiles) {
    Write-Host "    - $file"
}

Write-Host ""
Write-Host "git status --short"
git status --short
if ($LASTEXITCODE -ne 0) {
    throw "git status failed."
}

if ([string]::IsNullOrWhiteSpace($Subject)) {
    $Subject = "chore($Scope): $Verb slice#$SliceNumber artifacts"
}

Write-Host ""
Write-Host "Commit subject:"
Write-Host "  $Subject"

if ($DryRun) {
    Write-Host ""
    Write-Host "Dry run plan:"
    Write-Host "  git add -- <files>"
    Write-Host "  git commit -F <temp-file>"
    Write-Host ""
    Write-Host "Dry run enabled. Commit was not created."
    if (-not [string]::IsNullOrWhiteSpace($Body)) {
        Write-Host "Commit body preview:"
        Write-Host $Body
    }
    exit 0
}

Write-Host ""
Write-Host "git add -- <files>"
git add -- @selectedFiles
if ($LASTEXITCODE -ne 0) {
    throw "git add failed."
}

$tempFile = [System.IO.Path]::GetTempFileName()
try {
    $message = $Subject
    if (-not [string]::IsNullOrWhiteSpace($Body)) {
        $message = "$Subject`n`n$Body"
    }
    Set-Content -LiteralPath $tempFile -Value $message -Encoding ascii

    Write-Host ""
    Write-Host "git commit -F <temp-file>"
    git commit -F $tempFile
    $commitExitCode = $LASTEXITCODE

    if ($commitExitCode -ne 0) {
        Write-Warning "Commit failed. Inspect git output above."
        exit $commitExitCode
    }
}
finally {
    Remove-Item -LiteralPath $tempFile -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "git log -1 --oneline"
git log -1 --oneline
if ($LASTEXITCODE -ne 0) {
    Write-Warning "git log -1 --oneline failed."
    exit 1
}

$finalSubject = git log -1 --pretty=%s
if ($LASTEXITCODE -eq 0 -and $finalSubject -like "Made-with:*") {
    Write-Warning "Commit subject appears rewritten to 'Made-with: ...'. Check commit hooks/config."
}
