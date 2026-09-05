$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Provisioning.Core.ps1')
function Assert([bool]$Condition) { if (!$Condition) { throw 'synthetic_assertion_failed' } }
function Rejects([scriptblock]$Action) { $failed = $false; try { $null = & $Action } catch { $failed = $true }; Assert $failed }
$endpoint = 'https://hanni-personal-relay-v2.fixture.workers.dev'
$source = '00000000-0000-0000-0000-000000000001'
$script:sequence = 0
$syntheticRandom = { param($n) $script:sequence++; $bytes = New-Object byte[] $n; for ($i=0; $i -lt $n; $i++) { $bytes[$i] = $script:sequence }; return ,$bytes }
$passed = 0
try {
    Rejects { Test-RelayInputs $endpoint $source $false $true }
    Rejects { Test-RelayInputs $endpoint $source $true $false }
    Rejects { Test-RelayInputs 'http://fixture.invalid' $source $true $true }
    Rejects { Test-RelayInputs $endpoint 'not-an-identity' $true $true }
    Assert ((Test-RelayInputs $endpoint $source $true $true) -ceq $endpoint)
    $passed++

    $bundle = New-RelayBundle $endpoint $source $syntheticRandom
    Assert-RelayBundle $bundle
    Assert ($bundle.devices.Count -eq 4)
    Assert (@($bundle.devices.config.token | Select-Object -Unique).Count -eq 4)
    Assert (@($bundle.devices.config.device_id | Select-Object -Unique).Count -eq 4)
    Assert (@($bundle.devices.config.key | Select-Object -Unique).Count -eq 1)
    Assert (@($bundle.devices.config.sleep_source_store_id | Select-Object -Unique).Count -eq 1)
    $passed++

    $mapping = Get-RelayHashMapping $bundle
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $expected = ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::ASCII.GetBytes($bundle.devices[0].config.token)))).Replace('-', '').ToLowerInvariant()
        Assert ($mapping[$bundle.devices[0].config.device_id] -ceq $expected)
    } finally { $sha.Dispose() }
    $json = $bundle | ConvertTo-Json -Depth 8 -Compress
    $hashJson = $mapping | ConvertTo-Json -Compress
    foreach ($device in $bundle.devices) { Assert (!$hashJson.Contains($device.config.token)); Assert (!$hashJson.Contains($device.config.key)) }
    $passed++

    $sealed = Protect-RelayBundle $json
    Assert ([Text.Encoding]::UTF8.GetString($sealed) -notlike '*fixture.workers.dev*')
    $restored = Unprotect-RelayBundle $sealed
    Assert (($restored | ConvertTo-Json -Depth 8 -Compress) -ceq $json)
    $tampered = [byte[]]$sealed.Clone(); $tampered[$tampered.Length - 1] = $tampered[$tampered.Length - 1] -bxor 1
    Rejects { Unprotect-RelayBundle $tampered }
    $passed++

    # Only synthetic ciphertext is written; never inspect the installed Hanni DB.
    $fixtureRoot = Join-Path $PSScriptRoot ('synthetic-output-' + [Guid]::NewGuid().ToString('N'))
    New-RelayPrivateDirectory $fixtureRoot
    $file = Join-Path $fixtureRoot 'synthetic.dpapi'
    Save-RelayBundle $file $bundle
    $before = [IO.File]::ReadAllBytes($file)
    Rejects { Save-RelayBundle $file $bundle }
    Assert ([Convert]::ToBase64String([IO.File]::ReadAllBytes($file)) -ceq [Convert]::ToBase64String($before))
    Assert (((Read-RelayBundle $file) | ConvertTo-Json -Depth 8 -Compress) -ceq $json)
    Assert ((Get-Acl -LiteralPath $fixtureRoot).AreAccessRulesProtected)
    $passed++

    $bad = $json | ConvertFrom-Json
    $bad.devices[1].config.token = $bad.devices[0].config.token
    Rejects { Assert-RelayBundle $bad }
    $bad = $json | ConvertFrom-Json
    $bad.devices[1].config.sleep_source_store_id = '00000000-0000-0000-0000-000000000002'
    Rejects { Assert-RelayBundle $bad }
    Rejects { Get-RelayPrivatePaths $PSScriptRoot }
    $passed++
    Write-Output ('synthetic_core_tests_passed=' + $passed)
} catch {
    Write-Output ('synthetic_core_tests_failed_after=' + $passed)
    exit 1
}
