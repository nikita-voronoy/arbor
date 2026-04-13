#!/usr/bin/env pwsh
# Arbor installer for Windows
# Usage: irm https://raw.githubusercontent.com/nikita-voronoy/arbor/main/install.ps1 | iex

$ErrorActionPreference = "Stop"
$Repo = "nikita-voronoy/arbor"
$Bin = "arbor"
$InstallDir = if ($env:ARBOR_INSTALL_DIR) { $env:ARBOR_INSTALL_DIR } else { "$env:USERPROFILE\.local\bin" }

Write-Host "arbor" -ForegroundColor Blue -NoNewline
Write-Host " — code navigation MCP server"
Write-Host ""

# Detect architecture
$Arch = if ([Environment]::Is64BitOperatingSystem) { "x86_64" } else { Write-Error "32-bit Windows is not supported"; exit 1 }
$Target = "$Arch-pc-windows-msvc"

# Find latest release
Write-Host "Fetching latest release..." -ForegroundColor DarkGray
try {
    $Release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ "User-Agent" = "arbor-installer" }
    $Tag = $Release.tag_name
} catch {
    $Tag = $null
}

if (-not $Tag) {
    # Try releases list (in case /latest doesn't work)
    try {
        $Releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" -Headers @{ "User-Agent" = "arbor-installer" }
        $Tag = $Releases[0].tag_name
    } catch {
        $Tag = $null
    }
}

if (-not $Tag) {
    Write-Host "No release found — falling back to cargo install..." -ForegroundColor DarkGray
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        cargo install --git "https://github.com/$Repo.git" arbor-mcp
        Write-Host "arbor installed via cargo" -ForegroundColor Green
    } else {
        Write-Error "cargo not found. Install Rust: https://rustup.rs"
    }
    exit
}

$Asset = "$Bin-$Target.zip"
$Url = "https://github.com/$Repo/releases/download/$Tag/$Asset"

Write-Host "Downloading $Tag for $Target..." -ForegroundColor DarkGray

$TmpDir = Join-Path ([System.IO.Path]::GetTempPath()) "arbor-install-$(Get-Random)"
New-Item -ItemType Directory -Path $TmpDir -Force | Out-Null
$ZipPath = Join-Path $TmpDir $Asset

try {
    Invoke-WebRequest -Uri $Url -OutFile $ZipPath -UseBasicParsing
} catch {
    Write-Host "Failed to download from $Url" -ForegroundColor Red
    Write-Host "Falling back to cargo install..." -ForegroundColor DarkGray
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        cargo install --git "https://github.com/$Repo.git" arbor-mcp
        Write-Host "arbor installed via cargo" -ForegroundColor Green
    } else {
        Write-Error "cargo not found. Install Rust: https://rustup.rs"
    }
    Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue
    exit
}

# Extract
Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

# Install
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
$ExePath = Join-Path $InstallDir "$Bin.exe"
Move-Item -Path (Join-Path $TmpDir "$Bin.exe") -Destination $ExePath -Force

# Cleanup
Remove-Item -Recurse -Force $TmpDir -ErrorAction SilentlyContinue

Write-Host "arbor $Tag installed → $ExePath" -ForegroundColor Green

# Check PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host ""
    Write-Host "Adding $InstallDir to PATH..." -ForegroundColor DarkGray
    [Environment]::SetEnvironmentVariable("Path", "$InstallDir;$UserPath", "User")
    $env:Path = "$InstallDir;$env:Path"
    Write-Host "Added to PATH (restart your terminal for it to take effect)" -ForegroundColor Green
}

# Configure Claude Code
if (Get-Command claude -ErrorAction SilentlyContinue) {
    Write-Host "Adding arbor to Claude Code..." -ForegroundColor DarkGray
    try {
        claude mcp add arbor -- arbor
        Write-Host "Registered with Claude Code." -ForegroundColor Green
    } catch {
        Write-Host "Already registered or claude mcp not available." -ForegroundColor DarkGray
    }
} else {
    Write-Host ""
    Write-Host "To add to Claude Code later:"
    Write-Host "  claude mcp add arbor -- arbor"
}

# --- Configure hooks to prefer arbor MCP ---
$Settings = "$env:USERPROFILE\.claude\settings.json"
$ClaudeMd = "$env:USERPROFILE\.claude\CLAUDE.md"

$HookEntry = @{
    matcher = "Grep|Glob"
    hooks = @(@{
        type = "command"
        command = 'echo ''{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"STOP: Prefer arbor MCP tools (search, references, skeleton, compact, boot) over Grep/Glob for code navigation. Fall back to Grep/Glob only for string literals, comments, or regex patterns."}}'''
        statusMessage = "Checking arbor preference..."
    })
}

New-Item -ItemType Directory -Path "$env:USERPROFILE\.claude" -Force | Out-Null

if (Test-Path $Settings) {
    $config = Get-Content $Settings -Raw | ConvertFrom-Json
    $hasHook = $false
    if ($config.hooks -and $config.hooks.PreToolUse) {
        foreach ($h in $config.hooks.PreToolUse) {
            if ($h.matcher -eq "Grep|Glob") { $hasHook = $true; break }
        }
    }
    if (-not $hasHook) {
        if (-not $config.hooks) { $config | Add-Member -NotePropertyName hooks -NotePropertyValue ([PSCustomObject]@{}) }
        if (-not $config.hooks.PreToolUse) { $config.hooks | Add-Member -NotePropertyName PreToolUse -NotePropertyValue @() }
        $config.hooks.PreToolUse = @($config.hooks.PreToolUse) + $HookEntry
        $config | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
        Write-Host "PreToolUse hook added to $Settings" -ForegroundColor Green
    } else {
        Write-Host "PreToolUse hook for Grep|Glob already configured — skipping." -ForegroundColor DarkGray
    }
} else {
    @{ hooks = @{ PreToolUse = @($HookEntry) } } | ConvertTo-Json -Depth 10 | Set-Content $Settings -Encoding UTF8
    Write-Host "Created $Settings with PreToolUse hook" -ForegroundColor Green
}

# --- Add CLAUDE.md instructions ---
$MarkerStart = "<!-- arbor:start -->"
$MarkerEnd = "<!-- arbor:end -->"

$ArborBlock = @"
$MarkerStart
## Code navigation: use arbor MCP first

When exploring a codebase or searching for code, **always prefer arbor MCP tools over grep/glob**:

- **Instead of grep for a symbol** → use ``mcp__arbor__search`` (fuzzy, deduped, ranked)
- **Instead of grep for "who calls X"** → use ``mcp__arbor__references`` (shows Definition, Call, TypeReference)
- **Instead of reading many files to understand structure** → use ``mcp__arbor__boot`` first, then ``mcp__arbor__skeleton`` or ``mcp__arbor__compact``
- **Instead of manually tracing dependencies** → use ``mcp__arbor__dependencies`` or ``mcp__arbor__impact``
- **After making changes to many files** → call ``mcp__arbor__reindex`` to refresh the index

Start every new project session with ``mcp__arbor__boot`` to get the project overview.

Always try arbor first, even for terms that might appear in comments or string literals. Fall back to grep/glob only when:
- arbor is not available
- arbor returned nothing useful and you need raw text/regex search as a last resort
$MarkerEnd
"@

if (Test-Path $ClaudeMd) {
    $content = Get-Content $ClaudeMd -Raw
    if ($content -match [regex]::Escape($MarkerStart)) {
        Write-Host "CLAUDE.md already contains arbor block — replacing..." -ForegroundColor DarkGray
        $pattern = "(?s)$([regex]::Escape($MarkerStart)).*?$([regex]::Escape($MarkerEnd))\r?\n?"
        $content = [regex]::Replace($content, $pattern, "")
        $content = $content.TrimEnd() + "`n`n$ArborBlock"
        $content | Set-Content $ClaudeMd -Encoding UTF8 -NoNewline
        Write-Host "Arbor block updated in $ClaudeMd" -ForegroundColor Green
    } else {
        Add-Content $ClaudeMd -Value "`n$ArborBlock"
        Write-Host "Arbor instructions added to $ClaudeMd" -ForegroundColor Green
    }
} else {
    $ArborBlock | Set-Content $ClaudeMd -Encoding UTF8
    Write-Host "Created $ClaudeMd with arbor instructions" -ForegroundColor Green
}

Write-Host ""
Write-Host "Try it: " -NoNewline
Write-Host "arbor --compact ." -ForegroundColor Blue
