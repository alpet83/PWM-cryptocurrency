[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [int]$SliceNumber,

    [Parameter(Mandatory = $true)]
    [ValidateSet("init", "fill-diff", "patch-manifest-numstat")]
    [string]$Mode,

    [string[]]$TouchedSymbols = @(),

    [string[]]$NoChangeAssertions = @(
        "tx-path/tx guards",
        "HTTP routes",
        "response fields",
        "error messages",
        "new API fields",
        "transport scheduling/backoff semantics"
    ),

    [string]$TaskArtifactPath = "tasks/20260424-sprint6-optimization.json",

    # By default, manifest scoped_diff_stat lists only product/tooling code paths (crates/**, tools/**).
    # Sprint markdown + task JSON churn is excluded to avoid self-referential, instantly-stale metadata.
    [switch]$IncludeArtifactPaths,

    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-IsoNow {
    return (Get-Date).ToString("o")
}

function Ensure-FileExists {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "File not found: $Path"
    }
}

function Ensure-ContainsMarker {
    param(
        [string]$FilePath,
        [string]$Content,
        [string]$Marker
    )
    if ($Content.IndexOf($Marker, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Target marker '$Marker' was not found in $FilePath. File is not modified."
    }
}

function Ensure-TouchedSymbolsForInit {
    if ($Mode -eq "init" -and $TouchedSymbols.Count -eq 0) {
        throw "Mode 'init' requires at least one -TouchedSymbols value."
    }
}

function New-ChecklistSection {
    param([int]$Slice)
@"
## Slice #$Slice Scope (automation draft)

- [ ] Define optimization-only scope for slice #$Slice.
- [ ] List expected touched symbols in task manifest.
- [ ] Confirm no behavior/API drift constraints.

## Done Criteria (slice #$Slice)

- [ ] Narrow diff is recorded and review-ready.
- [ ] `cargo fmt` completed.
- [ ] `cargo check -p pwmd` completed.

## Non-goals (slice #$Slice)

- [ ] Do not change transport/tx semantics.
- [ ] Do not expand public API contracts.
"@
}

function New-StatusSection {
    param([int]$Slice)
@"
## What was done (slice #$Slice, coding)

- Pending.

## Gate state (slice #$Slice coding)

- coding: pending
- fmt/check: pending

## Gate state (slice #$Slice testing)

- testing: pending

## Gate state (slice #$Slice review)

- review: pending
- orchestrator: pending
"@
}

function New-ReviewSection {
    param([int]$Slice)
@"
## Slice #$Slice Scope Proof (pre-review)

### Allowed files (strict list)

- `crates/pwmd/src/lib.rs`
- `docs/reviews/sprint-6-checklist.md`
- `docs/reviews/sprint-6-status-note.md`
- `docs/reviews/sprint-6-review-report.md`
- `docs/reviews/sprint-6-test-report.md`
- `tasks/20260424-sprint6-optimization.json`

### Touched symbols (slice-local)

- TODO

### Explicit no-change assertions

- TODO

### Code delta (manifest policy)

- В `review_evidence_manifest_sliceN.scoped_diff_stat` фиксируются **только** пути `crates/**` и `tools/**` (см. `slice-artifacts.ps1` по умолчанию). Строки по `docs/reviews/sprint-6-*.md` и task-json **не** входят: это процессный шум и self-reference, который моментально устаревает.
- В этом markdown-блоке **не** дублировать numstat по sprint-артефактам; детали кода — в manifest task JSON. Полный `git diff --numstat` в manifest только с `-IncludeArtifactPaths` при явной необходимости.

---

## Slice #$Slice Review Gate

### Verdict

PENDING

### Findings by severity

#### High
- Pending.

#### Medium
- Pending.

#### Low
- Pending.

### Recommendation

- Pending.
"@
}

function New-TestSection {
    param([int]$Slice)
@"
## Slice #$Slice Test Gate

Date: $(Get-Date -Format "yyyy-MM-dd")
Executor: `pwm-testing`

### Verdict

PENDING

### Commands and results

- `cargo test -p pwmd` -> PENDING

### Coverage notes

- Pending.

### Residual risks

- Pending.

---
"@
}

function Append-SectionIfMissing {
    param(
        [string]$Original,
        [string]$SectionHeader,
        [string]$SectionBody
    )

    $headerPattern = "(?m)^" + [regex]::Escape($SectionHeader) + "$"
    if ([regex]::IsMatch($Original, $headerPattern)) {
        return @{
            Changed = $false
            Content = $Original
        }
    }

    $trimmed = $Original.TrimEnd("`r", "`n")
    $newContent = "$trimmed`r`n`r`n$SectionBody`r`n"
    return @{
        Changed = $true
        Content = $newContent
    }
}

function ConvertTo-PrettyJson {
    param([object]$InputObject)
    return ($InputObject | ConvertTo-Json -Depth 100)
}

function Write-TextFileUtf8NoBom {
    param(
        [string]$Path,
        [string]$Content
    )
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

function Test-ScopedDiffStatPathIncluded {
    param(
        [string]$GitPath,
        [bool]$IncludeArtifactPaths
    )

    if ($IncludeArtifactPaths) {
        return $true
    }

    $normalized = $GitPath -replace '\\', '/'
    return $normalized.StartsWith("crates/", [System.StringComparison]::Ordinal) -or
        $normalized.StartsWith("tools/", [System.StringComparison]::Ordinal)
}

function Add-MemberIfMissing {
    param(
        [pscustomobject]$Object,
        [string]$Key,
        [object]$Value
    )
    if ($null -eq $Object.PSObject.Properties[$Key]) {
        $Object | Add-Member -NotePropertyName $Key -NotePropertyValue $Value
        return $true
    }
    return $false
}

function Build-ScopedDiffStat {
    param([bool]$IncludeArtifactPaths = $false)

    $lines = git diff --numstat HEAD
    if ($LASTEXITCODE -ne 0) {
        throw "git diff --numstat HEAD failed."
    }

    $result = New-Object 'System.Collections.Generic.List[string]'
    foreach ($line in $lines) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        $parts = $line -split "`t"
        if ($parts.Count -lt 3) {
            continue
        }
        $adds = $parts[0]
        $dels = $parts[1]
        $path = $parts[2]
        if (-not (Test-ScopedDiffStatPathIncluded -GitPath $path -IncludeArtifactPaths $IncludeArtifactPaths)) {
            continue
        }
        if ($adds -eq "-" -or $dels -eq "-") {
            $result.Add("$path | git diff --numstat vs HEAD: binary change")
        } else {
            $result.Add("$path | git diff --numstat vs HEAD: +$adds/-$dels")
        }
    }

    if ($result.Count -eq 0) {
        if ($IncludeArtifactPaths) {
            $result.Add("No changes in git diff --numstat HEAD.")
        } else {
            $result.Add("No crates/** or tools/** paths in git diff --numstat HEAD (pass -IncludeArtifactPaths to record sprint artifact paths).")
        }
    }
    return ,$result.ToArray()
}

function Build-ManifestScopedDiffStatHumanStrings {
    param([bool]$IncludeArtifactPaths = $false)

    $lines = git diff --numstat HEAD
    if ($LASTEXITCODE -ne 0) {
        throw "git diff --numstat HEAD failed."
    }

    $result = New-Object 'System.Collections.Generic.List[string]'
    foreach ($line in $lines) {
        if ([string]::IsNullOrWhiteSpace($line)) {
            continue
        }
        $parts = $line -split "`t", 3
        if ($parts.Count -lt 3) {
            continue
        }
        $adds = $parts[0]
        $dels = $parts[1]
        $path = $parts[2]
        if (-not (Test-ScopedDiffStatPathIncluded -GitPath $path -IncludeArtifactPaths $IncludeArtifactPaths)) {
            continue
        }
        if ($adds -eq "-" -or $dels -eq "-") {
            $result.Add("$path | modified (git diff --numstat vs HEAD: binary change)")
        } else {
            $addN = 0
            $delN = 0
            $addOk = [int]::TryParse($adds, [ref]$addN)
            $delOk = [int]::TryParse($dels, [ref]$delN)
            if (-not $addOk -or -not $delOk) {
                $result.Add("$path | modified (git diff --numstat vs HEAD: $adds insertions, $dels deletions)")
                continue
            }
            if ($delN -eq 0) {
                $result.Add("$path | modified ($addN insertions)")
            } else {
                $result.Add("$path | modified (git diff --numstat vs HEAD: $addN insertions, $delN deletions)")
            }
        }
    }

    if ($result.Count -eq 0) {
        if ($IncludeArtifactPaths) {
            $result.Add("No changes in git diff --numstat HEAD.")
        } else {
            $result.Add("No crates/** or tools/** paths in git diff --numstat HEAD (pass -IncludeArtifactPaths to record sprint artifact paths).")
        }
    }
    return ,$result.ToArray()
}

function Get-JsonStringCloseQuoteIndex {
    param(
        [string]$Raw,
        [int]$OpenQuoteIndex
    )

    $i = $OpenQuoteIndex + 1
    while ($i -lt $Raw.Length) {
        $c = $Raw[$i]
        if ($c -eq [char]92) {
            $i += 2
            if ($i -gt $Raw.Length) {
                throw "Unterminated JSON string escape near index $OpenQuoteIndex"
            }
            continue
        }
        if ($c -eq [char]34) {
            return $i
        }
        $i++
    }
    throw "Unterminated JSON string starting at index $OpenQuoteIndex"
}

function Find-ScopedDiffStatArrayBoundsRaw {
    param(
        [string]$Raw,
        [int]$SliceNumber
    )

    $manifestKey = "`"review_evidence_manifest_slice$SliceNumber`":"
    $mpos = $Raw.IndexOf($manifestKey, [System.StringComparison]::Ordinal)
    if ($mpos -lt 0) {
        throw "Manifest key '$manifestKey' was not found in raw task JSON."
    }

    $needle = "`"scoped_diff_stat`":"
    $spos = $Raw.IndexOf($needle, $mpos, [System.StringComparison]::Ordinal)
    if ($spos -lt 0) {
        throw "Field scoped_diff_stat was not found after manifest key for slice $SliceNumber."
    }

    $openBracket = $Raw.IndexOf('[', $spos)
    if ($openBracket -lt 0) {
        throw "scoped_diff_stat array '[' not found."
    }

    $depth = 0
    $i = $openBracket
    while ($i -lt $Raw.Length) {
        $c = $Raw[$i]
        if ($c -eq [char]34) {
            $closeQ = Get-JsonStringCloseQuoteIndex -Raw $Raw -OpenQuoteIndex $i
            $i = $closeQ + 1
            continue
        }
        if ($c -eq '[') {
            $depth++
        } elseif ($c -eq ']') {
            $depth--
            if ($depth -eq 0) {
                return @{
                    OpenBracket = $openBracket
                    CloseBracket = $i
                    ManifestKeyPos = $mpos
                }
            }
        }
        $i++
    }
    throw "Could not find closing ']' for scoped_diff_stat array (slice $SliceNumber)."
}

function Escape-JsonStringContent {
    param([string]$Text)
    return $Text.Replace('\', '\\').Replace('"', '\"')
}

function Format-ScopedDiffStatArrayRaw {
    param([string[]]$HumanLines)

    # Use LF only so patched task JSON stays consistent with repo `.gitattributes` (avoid CRLF numstat drift on Windows).
    $nl = "`n"
    $escaped = foreach ($h in $HumanLines) {
        $e = Escape-JsonStringContent -Text $h
        "      `"$e`""
    }
    return '[' + $nl + (($escaped -join (',' + $nl))) + $nl + '    ]'
}

function Replace-GeneratedAtAfterScopedDiffRaw {
    param(
        [string]$Raw,
        [int]$AfterArrayCloseBracket,
        [string]$IsoNow
    )

    $nextManifest = $Raw.IndexOf("`"review_evidence_manifest_slice", $AfterArrayCloseBracket + 1, [System.StringComparison]::Ordinal)
    $gpos = $Raw.IndexOf("`"generated_at`":", $AfterArrayCloseBracket + 1, [System.StringComparison]::Ordinal)
    if ($gpos -lt 0) {
        throw "generated_at was not found after scoped_diff_stat array."
    }
    if ($nextManifest -ge 0 -and $gpos -gt $nextManifest) {
        throw "generated_at match is ambiguous (appears to belong to a later manifest)."
    }

    $colon = $Raw.IndexOf(':', $gpos)
    if ($colon -lt 0) {
        throw "Malformed generated_at field (missing ':')."
    }
    $openValQuote = $Raw.IndexOf('"', $colon + 1)
    if ($openValQuote -lt 0) {
        throw "Malformed generated_at field (missing opening quote)."
    }
    $closeValQuote = Get-JsonStringCloseQuoteIndex -Raw $Raw -OpenQuoteIndex $openValQuote
    $before = $Raw.Substring(0, $openValQuote + 1)
    $tail = $Raw.Substring($closeValQuote)
    return $before + $IsoNow + $tail
}

function Update-TaskManifestNumstatRaw {
    param(
        [string]$Raw,
        [int]$SliceNumber,
        [string[]]$StatLines,
        [string]$IsoNow
    )

    $bounds = Find-ScopedDiffStatArrayBoundsRaw -Raw $Raw -SliceNumber $SliceNumber
    $newArray = Format-ScopedDiffStatArrayRaw -HumanLines $StatLines
    $prefix = $Raw.Substring(0, $bounds.OpenBracket)
    $suffix = $Raw.Substring($bounds.CloseBracket + 1)
    $merged = $prefix + $newArray + $suffix
    return Replace-GeneratedAtAfterScopedDiffRaw -Raw $merged -AfterArrayCloseBracket ($prefix.Length + $newArray.Length - 1) -IsoNow $IsoNow
}

Ensure-TouchedSymbolsForInit

$checklistPath = "docs/reviews/sprint-6-checklist.md"
$statusPath = "docs/reviews/sprint-6-status-note.md"
$reviewPath = "docs/reviews/sprint-6-review-report.md"
$testPath = "docs/reviews/sprint-6-test-report.md"

$markdownFiles = @(
    @{ Path = $checklistPath; Marker = "# Sprint 6 Checklist (optimization)" },
    @{ Path = $statusPath; Marker = "# Sprint 6 Status Note" },
    @{ Path = $reviewPath; Marker = "# Sprint 6 Review Report" },
    @{ Path = $testPath; Marker = "# Sprint 6 Test Report" }
)

Ensure-FileExists -Path $TaskArtifactPath
foreach ($entry in $markdownFiles) {
    Ensure-FileExists -Path $entry.Path
}

$taskRaw = Get-Content -LiteralPath $TaskArtifactPath -Raw
$taskObj = $taskRaw | ConvertFrom-Json

if ($Mode -eq "init") {
    $mdContents = @{}
    foreach ($entry in $markdownFiles) {
        $content = Get-Content -LiteralPath $entry.Path -Raw
        Ensure-ContainsMarker -FilePath $entry.Path -Content $content -Marker $entry.Marker
        $mdContents[$entry.Path] = $content
    }

    $updates = New-Object 'System.Collections.Generic.List[object]'

    $checkHeader = "## Slice #$SliceNumber Scope (automation draft)"
    $statusHeader = "## What was done (slice #$SliceNumber, coding)"
    $reviewHeader = "## Slice #$SliceNumber Scope Proof (pre-review)"
    $testHeader = "## Slice #$SliceNumber Test Gate"

    $checkResult = Append-SectionIfMissing -Original $mdContents[$checklistPath] -SectionHeader $checkHeader -SectionBody (New-ChecklistSection -Slice $SliceNumber)
    if ($checkResult.Changed) {
        $updates.Add(@{ Path = $checklistPath; Content = $checkResult.Content; Type = "markdown" })
    }

    $statusResult = Append-SectionIfMissing -Original $mdContents[$statusPath] -SectionHeader $statusHeader -SectionBody (New-StatusSection -Slice $SliceNumber)
    if ($statusResult.Changed) {
        $updates.Add(@{ Path = $statusPath; Content = $statusResult.Content; Type = "markdown" })
    }

    $reviewResult = Append-SectionIfMissing -Original $mdContents[$reviewPath] -SectionHeader $reviewHeader -SectionBody (New-ReviewSection -Slice $SliceNumber)
    if ($reviewResult.Changed) {
        $updates.Add(@{ Path = $reviewPath; Content = $reviewResult.Content; Type = "markdown" })
    }

    $testResult = Append-SectionIfMissing -Original $mdContents[$testPath] -SectionHeader $testHeader -SectionBody (New-TestSection -Slice $SliceNumber)
    if ($testResult.Changed) {
        $updates.Add(@{ Path = $testPath; Content = $testResult.Content; Type = "markdown" })
    }

    $manifestKey = "review_evidence_manifest_slice$SliceNumber"
    $codingKey = "mini_report_slice${SliceNumber}_coding"
    $testingKey = "mini_report_slice${SliceNumber}_testing"
    $reviewMiniKey = "mini_report_slice${SliceNumber}_review"

    $manifestAdded = Add-MemberIfMissing -Object $taskObj -Key $manifestKey -Value ([pscustomobject]@{
        allowed_files = @(
            "crates/pwmd/src/lib.rs",
            "docs/reviews/sprint-6-checklist.md",
            "docs/reviews/sprint-6-status-note.md",
            "docs/reviews/sprint-6-review-report.md",
            "docs/reviews/sprint-6-test-report.md",
            $TaskArtifactPath
        )
        touched_symbols = $TouchedSymbols
        asserted_unchanged = $NoChangeAssertions
        scoped_diff_stat = @(
            "TODO: tools/slice-artifacts.ps1 -Mode patch-manifest-numstat -SliceNumber $SliceNumber (code paths only; use -IncludeArtifactPaths for full tree)"
        )
        generated_at = "TODO: patch-manifest-numstat or fill-diff"
    })

    $codingAdded = Add-MemberIfMissing -Object $taskObj -Key $codingKey -Value ([pscustomobject]@{
        agent = "pwm-coding"
        done_at = "TODO"
        verdict = "pending"
        notes = @()
    })
    $testingAdded = Add-MemberIfMissing -Object $taskObj -Key $testingKey -Value ([pscustomobject]@{
        agent = "pwm-testing"
        done_at = "TODO"
        verdict = "pending"
        notes = @()
    })
    $reviewAdded = Add-MemberIfMissing -Object $taskObj -Key $reviewMiniKey -Value ([pscustomobject]@{
        agent = "pwm-review"
        done_at = "TODO"
        verdict = "pending"
        notes = @()
    })

    if ($manifestAdded -or $codingAdded -or $testingAdded -or $reviewAdded) {
        $updates.Add(@{
            Path = $TaskArtifactPath
            Content = ConvertTo-PrettyJson -InputObject $taskObj
            Type = "json"
        })
    }

    if ($updates.Count -eq 0) {
        Write-Host "No changes needed. All slice #$SliceNumber artifacts already exist."
        exit 0
    }

    Write-Host "Planned updates for slice #$SliceNumber (mode=init):"
    foreach ($u in $updates) {
        Write-Host "  - $($u.Path)"
    }

    if ($DryRun) {
        Write-Host "Dry run enabled. No files were written."
        exit 0
    }

    foreach ($u in $updates) {
        if ($u.Type -eq "json") {
            Write-TextFileUtf8NoBom -Path $u.Path -Content $u.Content
        } else {
            Write-TextFileUtf8NoBom -Path $u.Path -Content $u.Content
        }
    }

    Write-Host "Init mode completed for slice #$SliceNumber."
    exit 0
}

if ($Mode -eq "fill-diff") {
    $manifestKey = "review_evidence_manifest_slice$SliceNumber"
    $manifestProp = $taskObj.PSObject.Properties[$manifestKey]
    if ($null -eq $manifestProp) {
        throw "Manifest key '$manifestKey' was not found in $TaskArtifactPath. Run init mode first."
    }

    $manifest = $manifestProp.Value
    if ($null -eq $manifest.scoped_diff_stat) {
        throw "Manifest key '$manifestKey' does not contain scoped_diff_stat."
    }
    if ($null -eq $manifest.generated_at) {
        throw "Manifest key '$manifestKey' does not contain generated_at."
    }

    $newStat = Build-ScopedDiffStat -IncludeArtifactPaths $IncludeArtifactPaths.IsPresent
    $newGeneratedAt = Get-IsoNow

    Write-Host "Planned updates for slice #$SliceNumber (mode=fill-diff; IncludeArtifactPaths=$IncludeArtifactPaths):"
    Write-Host "  - $TaskArtifactPath::$manifestKey.scoped_diff_stat"
    foreach ($line in $newStat) {
        Write-Host "      $line"
    }
    Write-Host "  - $TaskArtifactPath::$manifestKey.generated_at = $newGeneratedAt"

    if ($DryRun) {
        Write-Host "Dry run enabled. No files were written."
        exit 0
    }

    $manifest.scoped_diff_stat = $newStat
    $manifest.generated_at = $newGeneratedAt
    $newJson = ConvertTo-PrettyJson -InputObject $taskObj
    Write-TextFileUtf8NoBom -Path $TaskArtifactPath -Content $newJson
    Write-Host "Fill-diff mode completed for slice #$SliceNumber."
    exit 0
}

if ($Mode -eq "patch-manifest-numstat") {
    Ensure-FileExists -Path $TaskArtifactPath
    $statLines = Build-ManifestScopedDiffStatHumanStrings -IncludeArtifactPaths $IncludeArtifactPaths.IsPresent
    $iso = Get-IsoNow
    $raw = [System.IO.File]::ReadAllText($TaskArtifactPath, [System.Text.UTF8Encoding]::new($false))
    $nextRaw = Update-TaskManifestNumstatRaw -Raw $raw -SliceNumber $SliceNumber -StatLines $statLines -IsoNow $iso

    Write-Host "Planned updates for slice #$SliceNumber (mode=patch-manifest-numstat; IncludeArtifactPaths=$IncludeArtifactPaths):"
    Write-Host "  - $TaskArtifactPath (review_evidence_manifest_slice$SliceNumber scoped_diff_stat + generated_at)"
    foreach ($s in $statLines) {
        Write-Host "      $s"
    }
    Write-Host "  - generated_at = $iso"

    if ($DryRun) {
        Write-Host "Dry run enabled. No files were written."
        exit 0
    }

    Write-TextFileUtf8NoBom -Path $TaskArtifactPath -Content $nextRaw
    Write-Host "patch-manifest-numstat mode completed for slice #$SliceNumber."
    exit 0
}

throw "Unsupported mode: $Mode"
