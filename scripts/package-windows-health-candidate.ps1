$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$release = Join-Path $root 'desktop/src-tauri/target/release'
$binary = Join-Path $release 'hanni.exe'
$configPath = Join-Path $root 'desktop/src-tauri/tauri.health-candidate.conf.json'
$config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
$version = $config.version
if ($version -ne '1.1.15') { throw 'Unexpected candidate version' }
$configHash = (& python -c 'import hashlib,json,pathlib,subprocess; p="desktop/src-tauri/tauri.health-candidate.conf.json"; b=subprocess.check_output(["git","show","HEAD:"+p]); assert json.loads(b) == json.loads(pathlib.Path(p).read_text()); print(hashlib.sha256(b).hexdigest())').Trim()
if ($LASTEXITCODE -ne 0 -or $configHash -notmatch '^[0-9a-f]{64}$') { throw 'Candidate config differs from committed source' }
$source = (& git -C $root rev-parse HEAD).Trim()
$tree = (& git -C $root rev-parse 'HEAD^{tree}').Trim()
if ($LASTEXITCODE -ne 0 -or $source -ne $env:GITHUB_SHA) { throw 'Build source SHA changed' }
$changed = @(& git -C $root diff --name-only HEAD)
if ($changed | Where-Object { $_ -ne 'desktop/src-tauri/Cargo.toml' }) { throw 'Build changed tracked source' }
# frontendDist embeds the source directory directly. Include ignored files in
# this check: an ignored generated JS file is still an additional build input.
$extraSources = @(& git -C $root ls-files --others -- desktop/src desktop/src-tauri/src scripts .github/workflows)
if ($LASTEXITCODE -ne 0 -or $extraSources.Count -gt 0) { throw 'Build added untracked source or embedded assets' }
$frontend = Join-Path $root 'desktop/src'
$reparsePoints = @(@(Get-Item -LiteralPath $frontend) + @(Get-ChildItem -LiteralPath $frontend -Recurse -Force) | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 })
if ($reparsePoints.Count -gt 0) { throw 'Embedded assets must not follow reparse points' }
if ($changed -contains 'desktop/src-tauri/Cargo.toml') {
    & python -c 'import pathlib,subprocess,tomllib; p="desktop/src-tauri/Cargo.toml"; assert tomllib.loads(subprocess.check_output(["git","show","HEAD:"+p],text=True)) == tomllib.loads(pathlib.Path(p).read_text())'
    if ($LASTEXITCODE -ne 0) { throw 'Build changed Cargo.toml semantics' }
}
$peVersion = [Diagnostics.FileVersionInfo]::GetVersionInfo($binary)
if ($peVersion.FileVersion -ne $version -or $peVersion.ProductVersion -ne $version) { throw 'PE version does not match effective candidate config' }
$stage = Join-Path $env:RUNNER_TEMP 'hanni-windows-health-candidate'
if (Test-Path -LiteralPath $stage) { throw 'Candidate staging path already exists' }
New-Item -ItemType Directory -Path $stage | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$visualStudio = (& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath).Trim()
if ($LASTEXITCODE -ne 0 -or -not $visualStudio) { throw 'Visual Studio native tools unavailable' }
$msvc = Get-ChildItem -LiteralPath (Join-Path $visualStudio 'VC/Tools/MSVC') -Directory | Sort-Object Name -Descending | Select-Object -First 1
$dumpbin = Join-Path $msvc.FullName 'bin/Hostx64/x64/dumpbin.exe'
$queue = [Collections.Generic.Queue[string]]::new()
$queue.Enqueue('hanni.exe')
$files = @()
$seen = @{}
while ($queue.Count -gt 0) {
    $name = $queue.Dequeue()
    if ($seen.ContainsKey($name.ToLowerInvariant())) { continue }
    $seen[$name.ToLowerInvariant()] = $true
    $path = Join-Path $release $name
    $headers = (& $dumpbin /HEADERS $path) -join "`n"
    if ($LASTEXITCODE -ne 0 -or $headers -notmatch '8664 machine \(x64\)') { throw 'Expected x64 PE image' }
    $depends = (& $dumpbin /DEPENDENTS $path) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect native dependencies' }
    $imports = @([regex]::Matches($depends, '(?im)^\s+([a-z0-9_.-]+\.dll)\s*$') | ForEach-Object { $_.Groups[1].Value.ToLowerInvariant() } | Sort-Object -Unique)
    $external = @()
    foreach ($dependency in $imports) {
        if (Test-Path -LiteralPath (Join-Path $release $dependency)) { $queue.Enqueue($dependency) }
        elseif ($dependency -match '^api-ms-win-' -or (Test-Path -LiteralPath (Join-Path $env:SystemRoot ('System32/' + $dependency)))) { $external += $dependency }
        else { throw 'Unresolved native dependency' }
    }
    Copy-Item -LiteralPath $path -Destination (Join-Path $stage $name)
    $files += [ordered]@{name=$name;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant();bytes=(Get-Item -LiteralPath $path).Length;imports=$imports;system_runtime_imports=$external}
}
$manifest = [ordered]@{
    schema='hanni.windows-health-candidate.v1'
    source_commit=$source
    source_tree=$tree
    version=$version
    config_sha256=$configHash
    updater_pub_sha256=(Get-FileHash -LiteralPath (Join-Path $root 'desktop/src-tauri/updater.pub') -Algorithm SHA256).Hash.ToLowerInvariant()
    build_profile='release'
    target='x86_64-pc-windows-msvc'
    pe_file_version=$peVersion.FileVersion
    pe_product_version=$peVersion.ProductVersion
    files=$files
    requirements=@('Windows x64 with listed system/runtime DLLs','WebView2 Evergreen for interactive UI','Existing database must contain zero cr-sqlite internal tables')
    signature_kind='Tauri Minisign detached signature; not Windows Authenticode'
    native_version_runtime_verified=$false
    runtime_and_autostart_tested=$false
    installed=$false
}
$manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stage 'verification.json') -Encoding UTF8
