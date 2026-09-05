# Windows PowerShell 5.1, STA. Proposal: do not run Prepare with real inputs before
# Free, candidate identity and selected source have actually been confirmed.
[CmdletBinding()]
param(
    [ValidateSet('Prepare', 'Check', 'Copy', 'PublishHashes')][string]$Action = 'Check',
    [string]$HanniDataDir = '',
    [string]$Endpoint = '',
    [string]$PrimarySourceStoreId = '',
    [switch]$FreePlanConfirmed,
    [switch]$CandidateIdentityVerified,
    [ValidateSet('windows', 's21', 's20', 'mac')][string]$Device = 'windows',
    [string]$NodeExe = '',
    [string]$WranglerEntry = '',
    [string]$WranglerConfig = ''
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$VerbosePreference = 'SilentlyContinue'
$DebugPreference = 'SilentlyContinue'
$bundle = $null; $serialized = $null; $code = $null; $hashJson = $null
try {
    . (Join-Path $PSScriptRoot 'Provisioning.Core.ps1')
    $canonicalEndpoint = Test-RelayInputs $Endpoint $PrimarySourceStoreId $FreePlanConfirmed.IsPresent $CandidateIdentityVerified.IsPresent
    $paths = Get-RelayPrivatePaths $HanniDataDir
    if ($Action -eq 'Prepare') {
        if ((Test-Path -LiteralPath $paths.Bundle) -or (Test-Path -LiteralPath $paths.NativeConfig) -or
            (Test-Path -LiteralPath $paths.Directory)) { throw 'relay_pairing_already_exists' }
        # All external prerequisites pass before any production randomness.
        $bundle = New-RelayBundle $canonicalEndpoint $PrimarySourceStoreId
        New-RelayPrivateDirectory $paths.Directory
        Save-RelayBundle $paths.Bundle $bundle
        Write-Output 'prepared_four_devices_dpapi_only'
    } else {
        $bundle = Read-RelayBundle $paths.Bundle
        foreach ($entry in $bundle.devices) {
            if ($entry.config.endpoint -cne $canonicalEndpoint -or $entry.config.sleep_source_store_id -cne $PrimarySourceStoreId) {
                throw 'relay_bundle_inputs_mismatch'
            }
        }
        if ($Action -eq 'Check') { Write-Output 'bundle_valid_four_devices_no_cloud_check' }
        elseif ($Action -eq 'Copy') {
            $selected = @($bundle.devices | Where-Object { $_.label -ceq $Device })
            if ($selected.Count -ne 1) { throw 'relay_device_invalid' }
            $code = $selected[0].config | ConvertTo-Json -Depth 5 -Compress
            Copy-RelayCode $code
            Write-Output 'selected_device_code_copied_history_and_roaming_disabled'
        } elseif ($Action -eq 'PublishHashes') {
            foreach ($path in @($NodeExe, $WranglerEntry, $WranglerConfig)) {
                if (!$path -or !(Test-Path -LiteralPath $path -PathType Leaf) -or $path.Contains('"')) { throw 'relay_tooling_required' }
            }
            $hashJson = Get-RelayHashMapping $bundle | ConvertTo-Json -Compress
            # These arguments are public paths. No config/key/token is in argv.
            $runner = Join-Path $PSScriptRoot 'publish-hashes.mjs'
            $start = [Diagnostics.ProcessStartInfo]::new()
            $start.FileName = [IO.Path]::GetFullPath($NodeExe)
            $start.Arguments = '"' + $runner + '" "' + [IO.Path]::GetFullPath($WranglerEntry) + '" "' + [IO.Path]::GetFullPath($WranglerConfig) + '" --free-confirmed --candidate-verified'
            $start.UseShellExecute = $false
            $start.CreateNoWindow = $true
            $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
            $start.RedirectStandardInput = $true
            $start.RedirectStandardOutput = $true
            $start.RedirectStandardError = $true
            $process = [Diagnostics.Process]::new()
            $process.StartInfo = $start
            try {
                [void]$process.Start()
                $stdout = $process.StandardOutput.ReadToEndAsync()
                $stderr = $process.StandardError.ReadToEndAsync()
                $process.StandardInput.Write($hashJson)
                $process.StandardInput.Close()
                if (!$process.WaitForExit(110000)) { $process.Kill(); throw 'relay_hash_upload_timeout' }
                $safeReply = $stdout.GetAwaiter().GetResult().Trim()
                $null = $stderr.GetAwaiter().GetResult()
                if ($process.ExitCode -ne 0 -or $safeReply -cne 'hashes_version_created_activation_required') { throw 'relay_hash_upload_failed' }
                Write-Output $safeReply
            } finally { $process.Dispose() }
        }
    }
} catch {
    # Never emit raw exceptions, paths, identifiers, stdout or the input bundle.
    Write-Output 'relay_provisioning_failed_no_sensitive_details'
    exit 1
} finally {
    $bundle = $null; $serialized = $null; $code = $null; $hashJson = $null
    # Managed strings cannot be guaranteed to have been wiped from process RAM.
}
