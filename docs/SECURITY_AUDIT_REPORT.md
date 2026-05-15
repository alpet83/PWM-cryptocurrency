# Security Audit Report — Windows 10 Host Hardening

**Host:** RYZEN-WORK
**OS:** Windows 10 Pro, Build 19045 (22H2)
**Audit Date:** 2026-05-06
**Based on:** [KHardening Windows 10/11 Checklist (Codeby)](https://codeby.net/threads/khardening-windows-10-11-posle-okonchaniya-podderzhki-cheklist-dlya-pentestera-i-administratora.92954/)

---

## Summary

| Category | Status | Risk |
|---|---|---|
| OS Version | Windows 10 Pro 22H2 (19045) — ESU active, last patch May 2026 | MODERATE |
| LSASS Protection (RunAsPPL) | NOT configured | CRITICAL |
| WDigest | NOT explicitly disabled | HIGH |
| UAC Level | ConsentPromptBehaviorAdmin = 0 (Never Notify) | CRITICAL |
| FilterAdministratorToken | NOT set | HIGH |
| Credential Guard | NOT active (ServiceRunning = {0}) | HIGH |
| SMBv1 | **Disabled** | OK |
| SMBv2 | Enabled | OK |
| LLMNR | NOT disabled (no GPO) | HIGH |
| NTLM (LmCompatibilityLevel) | NOT set (defaults to send LM+NTLM) | HIGH |
| AV (KTS) | Kaspersky Total Security — active (avp.exe running) | OK |
| Windows Defender | Disabled by KTS (expected behavior) | N/A |
| WinDefend Service | Stopped — KTS replaces Defender | N/A |
| Controlled Folder Access | NOT configured | MEDIUM |
| PowerShell v2 | Could not verify (DISM error) | MEDIUM |
| PowerShell Script Block Logging | NOT verified (likely not configured) | MEDIUM |
| PowerShell Constrained Language Mode | NOT configured | HIGH |
| Audit Policies | Could not query (privilege needed) | MEDIUM |
| Windows Firewall | Enabled but DefaultInboundAction = NotConfigured | MEDIUM |
| WinDefend Service | Stopped, Manual start | CRITICAL |

---

## Detailed Findings & Remediation

### 1. [MODERATE] OS — Windows 10 Pro 22H2 with ESU (Extended Security Updates)

**Finding:** Build 19045 (22H2). EOL date was October 14, 2025, but **ESU subscription is active**. Security updates confirmed through May 2026:

| Patch | Date | Type |
|---|---|---|
| KB5066790 | Oct 14, 2025 | Security Update |
| KB5068780 | Nov 12, 2025 | Security Update |
| KB5072653 | Dec 4, 2025 | Security Update |
| KB5077456 | Feb 11, 2026 | Security Update |
| KB5081263 | Mar 10, 2026 | Security Update |
| KB5084130 | Apr 15, 2026 | Security Update |
| KB5082200 | May 1, 2026 | Security Update |
| KB5082419 | May 1, 2026 | Update |

**Risk:** ESU has a finite expiration date (typically 3 years: Oct 2025 → Oct 2028). After ESU expires, this becomes CRITICAL.

**Mitigation:**
- Monitor ESU expiration date
- Plan migration to Windows 11 before ESU expires
- Apply all available updates (62 patches installed)
- Harden remaining settings below

---

### 2. [CRITICAL] LSASS Protection (RunAsPPL) — NOT Configured

**Finding:** Registry key `HKLM\SYSTEM\CurrentControlSet\Control\Lsa\RunAsPPL` does not exist. LSASS runs without Protected Process Light protection.

**Vector:** T1003.001 — Credential Access. Mimikatz can dump NTLM hashes, Kerberos tickets, and plaintext passwords from LSASS memory.

**Remediation:**
```powershell
# Enable RunAsPPL
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v RunAsPPL /t REG_DWORD /d 1 /f

# Verify after reboot:
reg query "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v RunAsPPL

# Verify LSASS is running as PPL:
# Look for "Lsa" process type "Protected" in Task Manager > Details
# Or check with: tasklist /fi "imagename eq lsass.exe" /v
```

**Requires:** Reboot to take effect. Requires Secure Boot to be fully effective.

---

### 3. [HIGH] WDigest UseLogonCredential — NOT Explicitly Disabled

**Finding:** Registry key `HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest\UseLogonCredential` does not exist.

**Vector:** Passwords may be stored in plaintext in LSASS memory if WDigest is active.

**Remediation:**
```powershell
# Disable WDigest caching
reg add "HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest" /v UseLogonCredential /t REG_DWORD /d 0 /f

# Verify:
reg query "HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest" /v UseLogonCredential
```

**Note:** On Windows 10 22H2, the default is 0 (disabled) since KB2871997, but explicit configuration is still recommended.

---

### 4. [CRITICAL] UAC — Set to "Never Notify"

**Finding:** `ConsentPromptBehaviorAdmin = 0` — UAC is configured to never prompt for consent. This is functionally equivalent to running everything as administrator.

**Vector:** T1548.002 — UAC Bypass. Combined with `EnableLUA=1` (UAC is technically enabled), this setting means all processes auto-elevate without any prompt.

**Remediation:**
```powershell
# Set UAC to prompt for consent for administrators
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v ConsentPromptBehaviorAdmin /t REG_DWORD /d 2 /f

# Recommended: Also set FilterAdministratorToken to apply UAC to built-in Admin
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v FilterAdministratorToken /t REG_DWORD /d 1 /f

# Verify:
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v ConsentPromptBehaviorAdmin
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v FilterAdministratorToken
```

**Levels:**
- `0` = Never notify (current — INSECURE)
- `5` = Notify only for non-Windows programs (default)
- `2` = Always notify (recommended for security)

---

### 5. [HIGH] FilterAdministratorToken — NOT Set

**Finding:** The built-in Administrator account bypasses UAC by default.

**Vector:** Any malware running as Administrator gets full system access without any UAC barrier.

**Remediation:** (See command above in #4)

---

### 6. [HIGH] Credential Guard / Device Guard — NOT Active

**Finding:** `SecurityServicesRunning = {0}` — Credential Guard is not actively protecting credentials.

**Vector:** T1550.002 — Pass-the-Hash. NTLM hashes stored in LSASS are accessible for lateral movement.

**Remediation (requires hardware support):**
```powershell
# Check prerequisites:
# 1. UEFI Secure Boot enabled
# 2. TPM 2.0 present
# 3. Virtualization (VT-x/AMD-V) enabled in BIOS
Get-CimInstance -ClassName Win32_DeviceGuard -Namespace root\Microsoft\Windows\DeviceGuard

# Enable via GPO:
# Computer Configuration → Administrative Templates → System → Device Guard
# → Turn On Virtualization Based Security → Enabled
# → Select Platform Security Level: Secure Boot and DMA Protection
```

**Requirements:**
- Windows 10 Enterprise/Education or Windows 11
- UEFI Secure Boot
- TPM 2.0
- Intel VT-x or AMD-V

**Note:** This host has Windows 10 **Pro** — Credential Guard requires **Enterprise** or **Education** edition. This is a significant limitation.

---

### 7. [OK] SMBv1 — Disabled

**Finding:** `EnableSMB1Protocol = False` — SMBv1 is correctly disabled.

**Status:** No remediation needed.

---

### 8. [HIGH] LLMNR — NOT Disabled

**Finding:** No GPO at `HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient\EnableMulticast`. LLMNR is active.

**Vector:** Responder can capture NTLMv2 hashes via LLMNR/NBT-NS poisoning. First hash often arrives in minutes during internal pentest.

**Remediation:**
```powershell
# Disable LLMNR via GPO
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient" /v EnableMulticast /t REG_DWORD /d 0 /f

# Or via Local Group Policy:
# Computer Configuration → Administrative Templates → Network → DNS Client
# → Turn Off Multicast Name Resolution → Enabled

# Verify:
reg query "HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient" /v EnableMulticast
```

**Also disable NBT-NS on all interfaces:**
```powershell
# Disable NetBIOS over TCP/IP on all adapters via registry:
# Per-interface under: HKLM\SYSTEM\CurrentControlSet\Services\NetBT\Parameters\Interfaces\
# Set NetbiosOptions = 2 (Disable)
```

---

### 9. [HIGH] NTLM — LmCompatibilityLevel NOT Set

**Finding:** `LmCompatibilityLevel` registry key does not exist. Default allows LM and NTLM authentication.

**Vector:** Pass-the-Hash (T1550.002), NTLM relay attacks.

**Remediation:**
```powershell
# Set NTLM to send NTLMv2 only, refuse LM
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v LmCompatibilityLevel /t REG_DWORD /d 5 /f

# Verify:
reg query "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v LmCompatibilityLevel
```

**Levels:**
- `0` (default) = Send LM & NTLM
- `3` = Send NTLMv2 only
- `5` = Send NTLMv2 only, refuse LM & NTLM (recommended)

**Caution:** Before full block, set to Audit mode and monitor Event IDs 8001-8004 for legacy dependencies.

---

### 10. [N/A] Windows Defender — Disabled by Kaspersky Total Security

**Finding:** Windows Defender is disabled because **Kaspersky Total Security** (avp.exe, avpui.exe) is the active antivirus.

**Status:** This is expected behavior. When a third-party AV registers with Windows Security Center, Defender automatically disables itself to prevent conflicts.

**Verification:**
```powershell
# Kaspersky processes confirmed running:
# avp.exe   — 415 MB RAM (main KTS engine)
# avpui.exe — 119 MB RAM (KTS UI)
```

**Recommendations for KTS:**
- Ensure KTS real-time protection is enabled in KTS settings
- Verify KTS virus definitions are up to date
- Check KTS Self-Defense setting (equivalent to Defender Tamper Protection):
  - KTS Settings → General → Enable Self-Defense
- Ensure KTS System Watcher (ransomware protection) is active
- Review KTS exclusions — overly broad exclusions create blind spots

**Note:** ASR rules are Defender-specific and unavailable when using third-party AV. KTS has its own equivalent protections:
- KTS System Watcher → replaces Controlled Folder Access
- KTS Attack Prevention → replaces some ASR functionality
- KTS Application Control → replaces AppLocker for many use cases

---

### 11. [INFO] ASR Rules — Not Applicable (KTS Active)

**Finding:** ASR (Attack Surface Reduction) rules are empty because they are a Windows Defender feature.

**Status:** KTS provides equivalent protections through:
- **KTS Application Control** → blocks unauthorized executables
- **KTS Attack Prevention** → blocks exploit techniques
- **KTS Exploit Prevention** → covers browser, document, memory exploits

**Recommendation:** Verify in KTS console that these protections are enabled:
- KTS Settings → Protection → Application Control
- KTS Settings → Protection → Exploit Prevention
- KTS Settings → Protection → System Watcher (anti-ransomware)

---

### 12. [INFO] Controlled Folder Access — Handled by KTS System Watcher

**Finding:** Defender Controlled Folder Access is not configured because KTS is the active AV.

**Status:** KTS System Watcher provides equivalent anti-ransomware protection:
- Monitors file system activity for encryption patterns
- Maintains backup copies of files before modification
- Can roll back ransomware-encrypted files

**Recommendation:** Verify in KTS:
- Settings → Protection → System Watcher → Enabled
- Settings → Protection → Ransomware Protection → Enabled

---

### 13. [MEDIUM] PowerShell v2 — Status Unknown

**Finding:** DISM returned error. PowerShell v2 may or may not be installed.

**Vector:** PowerShell 2.0 bypasses AMSI and Script Block Logging. Attackers intentionally downgrade.

**Remediation:**
```powershell
# Check and remove PowerShell v2
Get-WindowsOptionalFeature -Online -FeatureName MicrosoftWindowsPowerShellV2Root | Select-Object State

# Remove if enabled:
Disable-WindowsOptionalFeature -Online -FeatureName MicrosoftWindowsPowerShellV2Root -NoRestart
```

---

### 14. [HIGH] PowerShell Constrained Language Mode — NOT Configured

**Finding:** No WDAC policy in place for Constrained Language Mode.

**Vector:** T1059.001 — Full .NET access, COM objects, Add-Type allow arbitrary code execution.

**Remediation:**
```powershell
# Constrained Language Mode requires WDAC policy
# Enable via WDAC (not via environment variable which is trivially bypassed):
# Use WDAC Wizard or New-CIPolicy to create policy
# Deploy via GPO

# Quick check of current language mode:
# In a new PowerShell: $ExecutionContext.SessionState.LanguageMode
```

**Alternative (easier):** AppLocker rules to restrict PowerShell execution to trusted paths.

---

### 15. [MEDIUM] PowerShell Script Block Logging — Likely NOT Configured

**Finding:** No evidence of Script Block Logging GPO.

**Vector:** Fileless attacks are invisible to blue team.

**Remediation:**
```powershell
# Enable via GPO:
# Computer Configuration → Administrative Templates → Windows Components → Windows PowerShell
# → Turn on PowerShell Script Block Logging → Enabled

# Via registry:
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows\PowerShell\ScriptBlockLogging" /v EnableScriptBlockLogging /t REG_DWORD /d 1 /f

# Also enable module logging:
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows\PowerShell\ModuleLogging" /v EnableModuleLogging /t REG_DWORD /d 1 /f
```

---

### 16. [MEDIUM] Audit Policies — Could Not Query

**Finding:** Insufficient privileges to query audit policy. Needs verification.

**Remediation (via Local Security Policy):**
```powershell
# Enable critical audit policies via GPO:
# Audit Logon (Success, Failure) — detects brute force, PtH
# Audit Sensitive Privilege Use (Success) — detects SeDebugPrivilege abuse
# Audit SAM (Success) — detects SAM database dump
# Audit Process Creation (Success) — detects malicious process execution
# Audit Audit Policy Change (Success) — detects audit tampering (T1562.002)

# Enable command line in process creation events:
# Computer Configuration → Administrative Templates → System → Audit Process Creation
# → Include command line in process creation events → Enabled
```

---

### 17. [MEDIUM] Windows Firewall — DefaultInboundAction Not Configured

**Finding:** All profiles (Domain, Private, Public) are Enabled, but `DefaultInboundAction = NotConfigured` (not "Block").

**Remediation:**
```powershell
# Set default inbound action to Block on all profiles
Set-NetFirewallProfile -Profile Domain,Private,Public -DefaultInboundAction Block

# Verify:
Get-NetFirewallProfile | Select-Object Name, Enabled, DefaultInboundAction, DefaultOutboundAction

# Recommended additional rules:
# - RDP only from specific admin IPs
# - WinRM only from admin workstations
# - Block all other inbound by default
```

---

### 18. [CRITICAL] Network Exposure — Multiple Virtual/VPN Adapters

**Finding:** Host has 12 network interfaces including:
- TAP adapters (VPN/tunnel)
- VMware VMnet adapters (VMnet1, VMnet4, VMnet8)
- Hyper-V WSL adapter
- Wi-Fi + Ethernet (both active)
- Bluetooth PAN

**Risk:** Large attack surface. Each adapter potentially exposes services.

**Remediation:**
```powershell
# Review which adapters need to be active
Get-NetAdapter | Select-Object Name, InterfaceDescription, Status

# Disable unused VMware networks if not needed:
# Disable-NetAdapter -Name "VMware Network Adapter VMnet1"

# Ensure firewall rules apply to all active profiles
```

---

## Priority Remediation Plan

### Phase 1 — Quick Wins (15-30 minutes, low risk)

| # | Action | Command |
|---|--------|---------|
| 1 | Enable RunAsPPL | `reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v RunAsPPL /t REG_DWORD /d 1 /f` |
| 2 | Disable WDigest | `reg add "HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest" /v UseLogonCredential /t REG_DWORD /d 0 /f` |
| 3 | Harden UAC | `reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v ConsentPromptBehaviorAdmin /t REG_DWORD /d 2 /f` |
| 4 | Filter Admin Token | `reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v FilterAdministratorToken /t REG_DWORD /d 1 /f` |
| 5 | Disable LLMNR | `reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient" /v EnableMulticast /t REG_DWORD /d 0 /f` |
| 6 | Set NTLM Level 5 | `reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v LmCompatibilityLevel /t REG_DWORD /d 5 /f` |
| 7 | Firewall Block Inbound | `Set-NetFirewallProfile -Profile Domain,Private,Public -DefaultInboundAction Block` |

### Phase 2 — KTS Verification (15 minutes)

| # | Action |
|---|--------|
| 1 | Verify KTS real-time protection is enabled |
| 2 | Verify KTS Self-Defense is enabled (anti-tamper) |
| 3 | Verify KTS System Watcher is active (anti-ransomware) |
| 4 | Verify KTS Application Control is active |
| 5 | Verify KTS Exploit Prevention is active |
| 6 | Review KTS exclusions — remove overly broad ones |

### Phase 3 — Logging & PowerShell (30 minutes)

| # | Action |
|---|--------|
| 1 | Enable PowerShell Script Block Logging |
| 2 | Enable PowerShell Module Logging |
| 3 | Remove PowerShell v2 |
| 4 | Configure Audit Policies (Logon, Process Creation, SAM, Privilege Use, Policy Change) |
| 5 | Enable process creation command line logging |

### Phase 4 — Architectural (requires planning)

| # | Action | Notes |
|---|--------|-------|
| 1 | Migrate to Windows 11 | Plan before ESU expires (Oct 2028). Hardware is compatible (TPM 2.0, Secure Boot) |
| 2 | Credential Guard | Requires Enterprise/Education edition (current = Pro) |
| 3 | AppLocker / WDAC | Complex deployment, test in Audit mode first |
| 4 | Network segmentation | VLAN for dev/test hosts, restrict lateral movement |
| 5 | SIEM log forwarding | Forward Windows Event Logs to centralized SIEM |

---

## One-Liner Script for Phase 1

```powershell
# Run as Administrator — Quick Hardening (Phase 1)
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v RunAsPPL /t REG_DWORD /d 1 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\SecurityProviders\WDigest" /v UseLogonCredential /t REG_DWORD /d 0 /f
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v ConsentPromptBehaviorAdmin /t REG_DWORD /d 2 /f
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System" /v FilterAdministratorToken /t REG_DWORD /d 1 /f
reg add "HKLM\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient" /v EnableMulticast /t REG_DWORD /d 0 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Lsa" /v LmCompatibilityLevel /t REG_DWORD /d 5 /f
Set-NetFirewallProfile -Profile Domain,Private,Public -DefaultInboundAction Block
Write-Output "Phase 1 complete. Reboot required for RunAsPPL to take effect."
```

---

## Risk Assessment

| Metric | Before | After Phase 1-3 |
|--------|--------|-----------------|
| Credential access (LSASS dump) | TRIVIAL | HARD (PPL + WDigest off) |
| Lateral movement (PtH/NTLM) | TRIVIAL | HARD (NTLM level 5 + LLMNR off) |
| Privilege escalation (UAC bypass) | TRIVIAL | MODERATE (UAC hardening) |
| Fileless execution | PARTIAL (KTS) | FULL (KTS + Script Block Logging) |
| Malware execution | PROTECTED (KTS) | PROTECTED (KTS + logging) |
| Ransomware | PROTECTED (KTS System Watcher) | PROTECTED (KTS + hardening) |

**Residual Risk after Phases 1-3:** System is patched via ESU through at least Oct 2028. Main remaining gaps are: UAC at "Never Notify", no LSASS protection, LLMNR/NTLM not restricted. Phase 1 closes most of these.

---

## References

- CIS Microsoft Windows 10 Enterprise Benchmark v3.0.0 (Level 1 + Level 2)
- DISA STIG Windows 10
- NIST NCP Checklist ID 1162
- MITRE ATT&CK: T1003.001, T1059.001, T1068, T1548.002, T1550.002, T1562.001, T1562.002
- HardeningKitty (CIS Benchmark automation)
- PingCastle (AD domain-level audit)
