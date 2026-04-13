#!/usr/bin/env pwsh
# Arbor uninstaller for Windows

$ErrorActionPreference = "Stop"
$Bin = "arbor"
$InstallDir = if ($env:ARBOR_INSTALL_DIR) { $env:ARBOR_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }

Write-Host "arbor" -ForegroundColor Blue -NoNewline
Write-Host " — uninstaller"
Write-Host ""

# --- Remove binary ---
$ExePath = Join-Path $InstallDir "$Bin.exe"
if (Test-Path $ExePath) {
    Remove-Item $ExePath -Force
    Write-Host "Removed $ExePath" -ForegroundColor Green
} else {
    $found = Get-Command arbor -ErrorAction SilentlyContinue
    if ($found) {
        Write-Host "Found arbor at $($found.Source) (not in expected $InstallDir)" -ForegroundColor DarkGray
        Write-Host "Remove it manually: Remove-Item `"$($found.Source)`"" -ForegroundColor DarkGray
    } else {
        Write-Host "arbor binary not found — skipping." -ForegroundColor DarkGray
    }
}

# --- Remove Claude Code MCP registration ---
if (Get-Command claude -ErrorAction SilentlyContinue) {
    Write-Host "Removing arbor from Claude Code..." -ForegroundColor DarkGray
    try {
        claude mcp remove arbor
        Write-Host "Unregistered from Claude Code." -ForegroundColor Green
    } catch {
        Write-Host "Not registered or claude mcp not available." -ForegroundColor DarkGray
    }
}

# --- Remove PreToolUse hook from settings.json ---
$Settings = "$env:USERPROFILE\.claude\settings.json"

if (Test-Path $Settings) {
    $config = Get-Content $Settings -Raw | ConvertFrom-Json
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
            # Remove hooks object if empty
            $remainingHookProps = @($config.hooks.PSObject.Properties)
            if ($remainingHookProps.Count -eq 0) {
                $config.PSObject.Properties.Remove("hooks")
            }
        }
    }

    if ($modified) {
        $config | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
        Write-Host "PreToolUse hook removed from $Settings" -ForegroundColor Green
    } else {
        Write-Host "No arbor PreToolUse hook found — skipping." -ForegroundColor DarkGray
    }
}

# --- Remove arbor block from CLAUDE.md ---
$ClaudeMd = "$env:USERPROFILE\.claude\CLAUDE.md"
$MarkerStart = "<!-- arbor:start -->"
$MarkerEnd = "<!-- arbor:end -->"

if (Test-Path $ClaudeMd) {
    $content = Get-Content $ClaudeMd -Raw
    if ($content -match [regex]::Escape($MarkerStart)) {
        Write-Host "Removing arbor block from $ClaudeMd..." -ForegroundColor DarkGray
        $pattern = "(?s)\r?\n?$([regex]::Escape($MarkerStart)).*?$([regex]::Escape($MarkerEnd))\r?\n?"
        $content = [regex]::Replace($content, $pattern, "`n")
        $content = $content.TrimEnd()

        if ([string]::IsNullOrWhiteSpace($content)) {
            Remove-Item $ClaudeMd -Force
            Write-Host "Removed empty $ClaudeMd" -ForegroundColor Green
        } else {
            "$content`n" | Set-Content $ClaudeMd -Encoding UTF8 -NoNewline
            Write-Host "Arbor block removed from $ClaudeMd" -ForegroundColor Green
        }
    } else {
        Write-Host "No arbor markers found in $ClaudeMd — skipping." -ForegroundColor DarkGray
    }
}

Write-Host ""
Write-Host "arbor uninstalled." -ForegroundColor Green
