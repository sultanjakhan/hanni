Set-StrictMode -Version Latest
Add-Type -AssemblyName System.Security
$script:RelayBundleEntropy = [Text.Encoding]::UTF8.GetBytes('Hanni relay bootstrap bundle v1')
$script:RelayLabels = @('windows', 's21', 's20', 'mac')

function Test-RelayInputs([string]$Endpoint, [string]$Source, [bool]$FreeConfirmed, [bool]$IdentityVerified) {
    if (!$FreeConfirmed -or !$IdentityVerified) { throw 'relay_preconditions_required' }
    $uri = $null
    if (![Uri]::TryCreate($Endpoint, [UriKind]::Absolute, [ref]$uri) -or $uri.Scheme -cne 'https' -or
        $uri.UserInfo -or $uri.Query -or $uri.Fragment -or !$uri.IsDefaultPort -or $uri.AbsolutePath -ne '/' -or
        $uri.DnsSafeHost -notmatch '^hanni-personal-relay-v2\.[a-z0-9-]+\.workers\.dev$') { throw 'relay_endpoint_invalid' }
    $uuid = [Guid]::Empty
    if (![Guid]::TryParseExact($Source, 'D', [ref]$uuid) -or $uuid.ToString('D') -cne $Source) { throw 'relay_source_invalid' }
    return $uri.GetLeftPart([UriPartial]::Authority).ToLowerInvariant()
}

function ConvertTo-RelayBase64([byte[]]$Bytes) {
    return [Convert]::ToBase64String($Bytes).TrimEnd('=').Replace('+', '-').Replace('/', '_')
}

function Test-RelayKey([string]$Value) {
    if ($Value -cnotmatch '^[A-Za-z0-9_-]{43}$') { return $false }
    $decoded = $null
    try {
        $decoded = [Convert]::FromBase64String($Value.Replace('-', '+').Replace('_', '/') + '=')
        return $decoded.Length -eq 32 -and (ConvertTo-RelayBase64 $decoded) -ceq $Value
    } catch { return $false }
    finally { if ($null -ne $decoded) { [Array]::Clear($decoded, 0, $decoded.Length) } }
}

function New-RelayRandom([int]$Length) {
    $bytes = New-Object byte[] $Length
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes); return ,$bytes } finally { $rng.Dispose() }
}

function New-RelayBundle([string]$Endpoint, [string]$Source, [scriptblock]$Random = ${function:New-RelayRandom}) {
    # Random injection exists solely for deterministic synthetic tests.
    $keyBytes = & $Random 32
    $idBytes = & $Random 16
    try {
        $key = ConvertTo-RelayBase64 $keyBytes
        $keyId = ConvertTo-RelayBase64 $idBytes
    } finally { [Array]::Clear($keyBytes, 0, $keyBytes.Length); [Array]::Clear($idBytes, 0, $idBytes.Length) }
    $devices = @()
    foreach ($label in $script:RelayLabels) {
        $tokenBytes = & $Random 32
        $deviceBytes = & $Random 16
        try {
            $devices += [ordered]@{ label = $label; config = [ordered]@{
                v = 1; endpoint = $Endpoint; device_id = ConvertTo-RelayBase64 $deviceBytes;
                key_id = $keyId; key = $key; token = ConvertTo-RelayBase64 $tokenBytes;
                enabled = $true; sleep_source_store_id = $Source
            } }
        } finally { [Array]::Clear($tokenBytes, 0, $tokenBytes.Length); [Array]::Clear($deviceBytes, 0, $deviceBytes.Length) }
    }
    return [ordered]@{ v = 1; created_at = [DateTime]::UtcNow.ToString('o'); devices = $devices }
}

function Assert-RelayBundle($Bundle) {
    if ($Bundle.v -ne 1 -or @($Bundle.devices).Count -ne 4) { throw 'relay_bundle_invalid' }
    $ids = @{}; $tokens = @{}; $first = $null; $labels = @{}
    foreach ($device in $Bundle.devices) {
        if ($device.label -cnotin $script:RelayLabels -or $labels.ContainsKey($device.label)) { throw 'relay_bundle_invalid' }
        $labels[$device.label] = $true
        $cfg = $device.config
        $canonical = Test-RelayInputs $cfg.endpoint $cfg.sleep_source_store_id $true $true
        if ($canonical -cne $cfg.endpoint -or $cfg.v -ne 1 -or $cfg.enabled -isnot [bool] -or
            $cfg.device_id -cnotmatch '^[A-Za-z0-9_-]{1,64}$' -or $cfg.key_id -cnotmatch '^[A-Za-z0-9_-]{1,64}$' -or
            !(Test-RelayKey $cfg.key) -or !(Test-RelayKey $cfg.token) -or $ids.ContainsKey($cfg.device_id) -or $tokens.ContainsKey($cfg.token)) { throw 'relay_bundle_invalid' }
        if ($null -ne $first -and ($cfg.endpoint -cne $first.endpoint -or $cfg.key -cne $first.key -or
            $cfg.key_id -cne $first.key_id -or $cfg.sleep_source_store_id -cne $first.sleep_source_store_id)) { throw 'relay_bundle_invalid' }
        $first = $cfg; $ids[$cfg.device_id] = $true; $tokens[$cfg.token] = $true
    }
}

function Get-RelayHashMapping($Bundle) {
    Assert-RelayBundle $Bundle
    $result = [ordered]@{}
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        foreach ($device in $Bundle.devices) {
            # Worker hashes the canonical bearer TEXT, not the decoded 32 bytes.
            $bytes = [Text.Encoding]::ASCII.GetBytes($device.config.token)
            try { $result[$device.config.device_id] = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
            finally { [Array]::Clear($bytes, 0, $bytes.Length) }
        }
        return $result
    } finally { $sha.Dispose() }
}

function Protect-RelayBundle([string]$Json) {
    $plain = [Text.Encoding]::UTF8.GetBytes($Json)
    try { return ,[Security.Cryptography.ProtectedData]::Protect($plain, $script:RelayBundleEntropy, [Security.Cryptography.DataProtectionScope]::CurrentUser) }
    finally { [Array]::Clear($plain, 0, $plain.Length) }
}

function Unprotect-RelayBundle([byte[]]$Bytes) {
    $plain = [Security.Cryptography.ProtectedData]::Unprotect($Bytes, $script:RelayBundleEntropy, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    try {
        if ($plain.Length -gt 32768) { throw 'relay_bundle_invalid' }
        $value = [Text.UTF8Encoding]::new($false, $true).GetString($plain) | ConvertFrom-Json
        Assert-RelayBundle $value
        return $value
    } finally { [Array]::Clear($plain, 0, $plain.Length) }
}

function Assert-RelayPath([string]$Path) {
    $item = [IO.DirectoryInfo]::new([IO.Path]::GetFullPath($Path))
    while ($null -ne $item) {
        if ($item.Exists -and (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) { throw 'relay_path_reparse_rejected' }
        $item = $item.Parent
    }
}

function Get-RelayPrivatePaths([string]$DataDir) {
    # Matches types::hanni_data_dir -> dirs::data_dir().join("Hanni") on Windows.
    $expected = [IO.Path]::GetFullPath((Join-Path ([Environment]::GetFolderPath([Environment+SpecialFolder]::ApplicationData)) 'Hanni'))
    $actual = [IO.Path]::GetFullPath($DataDir).TrimEnd('\')
    if ($actual -ine $expected.TrimEnd('\')) { throw 'relay_production_path_required' }
    Assert-RelayPath $actual
    $db = Join-Path $actual 'hanni.db'
    if (!(Test-Path -LiteralPath $db -PathType Leaf) -or ([IO.FileInfo]::new($db)).Length -lt 16) { throw 'relay_existing_database_required' }
    if (((Get-Item -LiteralPath $db).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'relay_path_reparse_rejected' }
    $dir = Join-Path $actual 'relay-pairing'
    Assert-RelayPath $dir
    return @{ Directory = $dir; Bundle = (Join-Path $dir 'bootstrap.dpapi'); NativeConfig = (Join-Path $actual 'cloud-relay.credentials') }
}

function New-RelayPrivateDirectory([string]$Path) {
    Assert-RelayPath $Path
    if (Test-Path -LiteralPath $Path) { throw 'relay_pairing_directory_exists' }
    $acl = New-Object Security.AccessControl.DirectorySecurity
    $acl.SetAccessRuleProtection($true, $false)
    foreach ($sid in @([Security.Principal.WindowsIdentity]::GetCurrent().User, [Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($sid, 'FullControl', 'ContainerInherit, ObjectInherit', 'None', 'Allow')
        $acl.AddAccessRule($rule)
    }
    $acl.SetOwner([Security.Principal.WindowsIdentity]::GetCurrent().User)
    [void][IO.Directory]::CreateDirectory($Path, $acl)
}

function Save-RelayBundle([string]$Path, $Bundle) {
    Assert-RelayBundle $Bundle
    if (Test-Path -LiteralPath $Path) { throw 'relay_bundle_exists' }
    Assert-RelayPath ([IO.Path]::GetDirectoryName($Path))
    $json = $Bundle | ConvertTo-Json -Depth 8 -Compress
    $sealed = Protect-RelayBundle $json
    try {
        $roundtrip = Unprotect-RelayBundle $sealed
        if (($roundtrip | ConvertTo-Json -Depth 8 -Compress) -cne $json) { throw 'relay_bundle_verification_failed' }
        $temp = $Path + '.new'
        $file = [IO.FileStream]::new($temp, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
        try { $file.Write($sealed, 0, $sealed.Length); $file.Flush($true) } finally { $file.Dispose() }
        # Atomic rename, never overwrite an existing pairing. Temp contains only DPAPI ciphertext.
        [IO.File]::Move($temp, $Path)
    } finally { $json = $null; $roundtrip = $null; [Array]::Clear($sealed, 0, $sealed.Length) }
}

function Read-RelayBundle([string]$Path) {
    Assert-RelayPath ([IO.Path]::GetDirectoryName($Path))
    $info = Get-Item -LiteralPath $Path
    if (($info.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 -or $info.Length -gt 65536) { throw 'relay_bundle_invalid' }
    $bytes = [IO.File]::ReadAllBytes($Path)
    try { return Unprotect-RelayBundle $bytes } finally { [Array]::Clear($bytes, 0, $bytes.Length) }
}

function Copy-RelayCode([string]$Code) {
    if ([Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') { throw 'relay_sta_required' }
    $null = [Windows.ApplicationModel.DataTransfer.DataPackage, Windows.ApplicationModel.DataTransfer, ContentType=WindowsRuntime]
    $null = [Windows.ApplicationModel.DataTransfer.ClipboardContentOptions, Windows.ApplicationModel.DataTransfer, ContentType=WindowsRuntime]
    $null = [Windows.ApplicationModel.DataTransfer.Clipboard, Windows.ApplicationModel.DataTransfer, ContentType=WindowsRuntime]
    $content = [Windows.ApplicationModel.DataTransfer.DataPackage]::new()
    $options = [Windows.ApplicationModel.DataTransfer.ClipboardContentOptions]::new()
    $options.IsAllowedInHistory = $false
    $options.IsRoamable = $false
    $content.SetText($Code)
    if (![Windows.ApplicationModel.DataTransfer.Clipboard]::SetContentWithOptions($content, $options)) { throw 'relay_clipboard_unavailable' }
    [Windows.ApplicationModel.DataTransfer.Clipboard]::Flush()
    # This API opts out of Windows history/roaming. Other clipboard managers and
    # device-control software are outside that guarantee; use a trusted session.
}
