#!/usr/bin/env pwsh
# Test suite for install.ps1 / uninstall.ps1 config logic
# Tests only the settings.json + CLAUDE.md portions (no binary download)

$ErrorActionPreference = "Stop"
$Script:Pass = 0
$Script:Fail = 0

function Pass($msg) { $Script:Pass++; Write-Host "  PASS: $msg" -ForegroundColor Green }
function Fail($msg) { $Script:Fail++; Write-Host "  FAIL: $msg" -ForegroundColor Red }

function Assert-Eq($actual, $expected, $msg) {
    if ($actual -eq $expected) { Pass $msg } else { Fail "$msg (expected '$expected', got '$actual')" }
}
function Assert-Contains($haystack, $needle, $msg) {
    if ($haystack -match [regex]::Escape($needle)) { Pass $msg } else { Fail "$msg (missing '$needle')" }
}
function Assert-NotContains($haystack, $needle, $msg) {
    if ($haystack -match [regex]::Escape($needle)) { Fail "$msg (found '$needle')" } else { Pass $msg }
}
function Assert-FileExists($path, $msg) {
    if (Test-Path $path) { Pass $msg } else { Fail "$msg ($path not found)" }
}
function Assert-FileNotExists($path, $msg) {
    if (Test-Path $path) { Fail "$msg ($path still exists)" } else { Pass $msg }
}

# --- Helpers that replicate install/uninstall config logic ---

function Run-Install($FakeHome) {
    $Settings = "$FakeHome\.claude\settings.json"
    $ClaudeMd = "$FakeHome\.claude\CLAUDE.md"

    $HookEntry = @{
        matcher = "Grep|Glob"
        hooks = @(@{
            type = "command"
            command = "echo arbor-hook"
            statusMessage = "Checking arbor preference..."
        })
    }

    New-Item -ItemType Directory -Path "$FakeHome\.claude" -Force | Out-Null

    # settings.json
    if (Test-Path $Settings) {
        $config = Get-Content $Settings -Raw | ConvertFrom-Json
        $hasHook = $false
        if ($config.hooks -and $config.hooks.PreToolUse) {
            foreach ($h in $config.hooks.PreToolUse) {
                if ($h.matcher -eq "Grep|Glob") { $hasHook = $true; break }
            }
        }
        if (-not $hasHook) {
            if (-not $config.hooks) { $config | Add-Member -NotePropertyName hooks -NotePropertyValue @{} }
            if (-not $config.hooks.PreToolUse) { $config.hooks | Add-Member -NotePropertyName PreToolUse -NotePropertyValue @() }
            $config.hooks.PreToolUse += $HookEntry
            $config | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
        }
    } else {
        @{ hooks = @{ PreToolUse = @($HookEntry) } } | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
    }

    # CLAUDE.md
    $MarkerStart = "<!-- arbor:start -->"
    $MarkerEnd = "<!-- arbor:end -->"
    $ArborBlock = @"
$MarkerStart
## Code navigation: use arbor MCP first
Test content here.
$MarkerEnd
"@

    if (Test-Path $ClaudeMd) {
        $content = Get-Content $ClaudeMd -Raw
        if ($content -match [regex]::Escape($MarkerStart)) {
            $pattern = "(?s)$([regex]::Escape($MarkerStart)).*?$([regex]::Escape($MarkerEnd))\r?\n?"
            $content = [regex]::Replace($content, $pattern, "")
            $content = $content.TrimEnd() + "`n`n$ArborBlock"
            $content | Set-Content $ClaudeMd -Encoding UTF8 -NoNewline
        } else {
            Add-Content $ClaudeMd -Value "`n$ArborBlock"
        }
    } else {
        $ArborBlock | Set-Content $ClaudeMd -Encoding UTF8
    }
}

function Run-Uninstall($FakeHome) {
    $Settings = "$FakeHome\.claude\settings.json"
    $ClaudeMd = "$FakeHome\.claude\CLAUDE.md"
    $MarkerStart = "<!-- arbor:start -->"
    $MarkerEnd = "<!-- arbor:end -->"

    # settings.json
    if (Test-Path $Settings) {
        $raw = Get-Content $Settings -Raw
        $config = $raw | ConvertFrom-Json
        $modified = $false

        if ($config.hooks -and $config.hooks.PreToolUse) {
            $before = @($config.hooks.PreToolUse).Count
            $filtered = @($config.hooks.PreToolUse | Where-Object { $_.matcher -ne "Grep|Glob" })
            if ($filtered.Count -lt $before) {
                $modified = $true
                if ($filtered.Count -eq 0) {
                    $config.hooks.PSObject.Properties.Remove("PreToolUse")
                } else {
                    $config.hooks.PreToolUse = $filtered
                }
                $remainingHookProps = @($config.hooks.PSObject.Properties)
                if ($remainingHookProps.Count -eq 0) {
                    $config.PSObject.Properties.Remove("hooks")
                }
            }
        }

        if ($modified) {
            $config | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
        }
    }

    # CLAUDE.md
    if (Test-Path $ClaudeMd) {
        $content = Get-Content $ClaudeMd -Raw
        if ($content -match [regex]::Escape($MarkerStart)) {
            $pattern = "(?s)\r?\n?$([regex]::Escape($MarkerStart)).*?$([regex]::Escape($MarkerEnd))\r?\n?"
            $content = [regex]::Replace($content, $pattern, "`n")
            $content = $content.TrimEnd()

            if ([string]::IsNullOrWhiteSpace($content)) {
                Remove-Item $ClaudeMd -Force
            } else {
                "$content`n" | Set-Content $ClaudeMd -Encoding UTF8 -NoNewline
            }
        }
    }
}

# --- Tests ---

$TmpRoot = Join-Path ([System.IO.Path]::GetTempPath()) "arbor-test-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpRoot -Force | Out-Null

try {

# ============================================================
Write-Host "=== Test 1: Clean install (no existing files) ==="
$Fake = "$TmpRoot\t1"
Run-Install $Fake

Assert-FileExists "$Fake\.claude\settings.json" "settings.json created"
Assert-FileExists "$Fake\.claude\CLAUDE.md" "CLAUDE.md created"
$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
$ptu = @($json.hooks.PreToolUse)
Assert-Eq $ptu[0].matcher "Grep|Glob" "hook matcher correct"
$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw
Assert-Contains $md "<!-- arbor:start -->" "CLAUDE.md has start marker"
Assert-Contains $md "<!-- arbor:end -->" "CLAUDE.md has end marker"
Assert-Contains $md "## Code navigation" "CLAUDE.md has section header"

# ============================================================
Write-Host "=== Test 2: Idempotency (install twice) ==="
Run-Install $Fake

$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
Assert-Eq $json.hooks.PreToolUse.Count 1 "no duplicate hooks after second install"
$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw
$count = ([regex]::Matches($md, "arbor:start")).Count
Assert-Eq $count 1 "no duplicate CLAUDE.md blocks after second install"

# ============================================================
Write-Host "=== Test 3: Install preserves existing settings ==="
$Fake = "$TmpRoot\t3"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
@'
{
  "skipDangerousModePermissionPrompt": true,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo logging"}]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{"type": "command", "command": "prettier --write"}]
      }
    ]
  },
  "permissions": {"allow": ["Bash(git:*)"]}
}
'@ | Set-Content "$Fake\.claude\settings.json" -Encoding UTF8

Run-Install $Fake
$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json

Assert-Eq $json.skipDangerousModePermissionPrompt $true "preserves top-level settings"
Assert-Eq $json.permissions.allow[0] "Bash(git:*)" "preserves permissions"
Assert-Eq $json.hooks.PreToolUse.Count 2 "two PreToolUse hooks total"
$ptu = @($json.hooks.PreToolUse)
Assert-Eq $ptu[0].matcher "Bash" "existing Bash hook preserved"
Assert-Eq $ptu[1].matcher "Grep|Glob" "arbor hook added"
Assert-Eq $json.hooks.PostToolUse.Count 1 "PostToolUse hooks preserved"

# ============================================================
Write-Host "=== Test 4: Install preserves existing CLAUDE.md content ==="
$Fake = "$TmpRoot\t4"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
@"
# Global instructions

## My custom rules

- Always use TypeScript
- Follow ESLint config
"@ | Set-Content "$Fake\.claude\CLAUDE.md" -Encoding UTF8

Run-Install $Fake
$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw

Assert-Contains $md "My custom rules" "existing content preserved"
Assert-Contains $md "Always use TypeScript" "existing rules preserved"
Assert-Contains $md "<!-- arbor:start -->" "arbor block added"

# ============================================================
Write-Host "=== Test 5: Install updates existing arbor block ==="
Run-Install $Fake

$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw
$count = ([regex]::Matches($md, "arbor:start")).Count
Assert-Eq $count 1 "only one arbor block after update"
Assert-Contains $md "My custom rules" "user content still preserved after update"

# ============================================================
Write-Host "=== Test 6: Uninstall removes only arbor additions ==="
$Fake = "$TmpRoot\t6"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
@'
{
  "skipDangerousModePermissionPrompt": true,
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo logging"}]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [{"type": "command", "command": "prettier --write"}]
      }
    ]
  }
}
'@ | Set-Content "$Fake\.claude\settings.json" -Encoding UTF8
@"
# My instructions

Custom user rules here.
"@ | Set-Content "$Fake\.claude\CLAUDE.md" -Encoding UTF8

Run-Install $Fake
Run-Uninstall $Fake

$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
Assert-Eq $json.skipDangerousModePermissionPrompt $true "top-level settings preserved after uninstall"
Assert-Eq $json.hooks.PreToolUse.Count 1 "only Bash hook remains"
$ptu = @($json.hooks.PreToolUse)
Assert-Eq $ptu[0].matcher "Bash" "Bash hook preserved"
Assert-Eq $json.hooks.PostToolUse.Count 1 "PostToolUse preserved after uninstall"

$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw
Assert-NotContains $md "<!-- arbor:start -->" "arbor start marker removed"
Assert-NotContains $md "<!-- arbor:end -->" "arbor end marker removed"
Assert-NotContains $md "mcp__arbor__" "arbor tool references removed"
Assert-Contains $md "Custom user rules here" "user CLAUDE.md content preserved"

# ============================================================
Write-Host "=== Test 8: Uninstall cleans up empty hooks object ==="
$Fake = "$TmpRoot\t8"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
'{}' | Set-Content "$Fake\.claude\settings.json" -Encoding UTF8

Run-Install $Fake
Run-Uninstall $Fake

$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
$hasHooks = [bool]($json.PSObject.Properties.Name -contains "hooks")
Assert-Eq $hasHooks $false "empty hooks object removed"

# ============================================================
Write-Host "=== Test 9: Uninstall removes CLAUDE.md if only arbor content ==="
$Fake = "$TmpRoot\t9"
Run-Install $Fake
Run-Uninstall $Fake

Assert-FileNotExists "$Fake\.claude\CLAUDE.md" "empty CLAUDE.md removed"

# ============================================================
Write-Host "=== Test 10: Uninstall preserves CLAUDE.md with user content ==="
$Fake = "$TmpRoot\t10"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
@"
# My instructions

Important stuff here.
"@ | Set-Content "$Fake\.claude\CLAUDE.md" -Encoding UTF8

Run-Install $Fake
Run-Uninstall $Fake

Assert-FileExists "$Fake\.claude\CLAUDE.md" "CLAUDE.md preserved when has user content"
$md = Get-Content "$Fake\.claude\CLAUDE.md" -Raw
Assert-Contains $md "Important stuff here" "user content preserved after uninstall"
Assert-NotContains $md "arbor" "no arbor references remain"

# ============================================================
Write-Host "=== Test 11: Uninstall on clean system (nothing to remove) ==="
$Fake = "$TmpRoot\t11"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
'{"model": "opus"}' | Set-Content "$Fake\.claude\settings.json" -Encoding UTF8

Run-Uninstall $Fake
$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
Assert-Eq $json.model "opus" "unrelated settings untouched"

# ============================================================
Write-Host "=== Test 12: Install with empty settings.json ==="
$Fake = "$TmpRoot\t12"
New-Item -ItemType Directory -Path "$Fake\.claude" -Force | Out-Null
'{}' | Set-Content "$Fake\.claude\settings.json" -Encoding UTF8

Run-Install $Fake
$json = Get-Content "$Fake\.claude\settings.json" -Raw | ConvertFrom-Json
$ptu = @($json.hooks.PreToolUse)
Assert-Eq $ptu[0].matcher "Grep|Glob" "hook added to empty settings"
$hasHooks = [bool]($json.PSObject.Properties.Name -contains "hooks")
Assert-Eq $hasHooks $true "hooks key present"

} finally {
    Remove-Item -Recurse -Force $TmpRoot -ErrorAction SilentlyContinue
}

# --- Results ---
Write-Host ""
Write-Host "Results: $($Script:Pass) passed, $($Script:Fail) failed"
if ($Script:Fail -gt 0) { exit 1 }
