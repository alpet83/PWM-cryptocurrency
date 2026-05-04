# slice-commit.ps1

Utility script for batching routine slice commits in one command.

## Usage

Run from the repository root:

```powershell
.\tools\slice-commit.ps1 -SliceNumber 6 -Files @("docs\reviews\sprint-6-review-report.md")
```

Use standard sprint-6 artifacts automatically:

```powershell
.\tools\slice-commit.ps1 -SliceNumber 10 -Files @("crates\pwmd\src\lib.rs") -IncludeStandardArtifacts
```

Pull file list from `review_evidence_manifest_sliceN.allowed_files` in the task artifact:

```powershell
.\tools\slice-commit.ps1 -SliceNumber 10 -FilesFromManifest
```

Combine both modes (manifest + explicit file overrides):

```powershell
.\tools\slice-commit.ps1 -SliceNumber 10 -Files @("crates\pwmd\src\lib.rs") -FilesFromManifest
```

With custom subject and body:

```powershell
.\tools\slice-commit.ps1 -SliceNumber 7 -Files @("tasks\slice7-note.txt","tasks\slice7-plan.txt") -Subject "chore(sprint6): record slice#7 artifacts" -Body "Attach note and implementation plan."
```

Preview without creating a commit:

```powershell
.\tools\slice-commit.ps1 -SliceNumber 8 -Files @("tasks\slice8-summary.txt") -DryRun
```

## Notes

- `-Files` is optional when `-IncludeStandardArtifacts` or `-FilesFromManifest` is used.
- Default task artifact path is `tasks/20260424-sprint6-optimization.json`.
- Override path or key prefix if needed:
  - `-TaskArtifactPath <path>`
  - `-ManifestKeyPrefix review_evidence_manifest_slice`
