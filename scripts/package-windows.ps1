param(
    [Parameter(Mandatory)][string]$SourceIdentity,
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory)][ValidateSet(13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29)][int]$StoreSchema,
    [string]$ReleaseDirectory = (Join-Path $PSScriptRoot '../target/release'),
    [string]$ExtensionDirectory = (Join-Path $PSScriptRoot '../apps/browser-extension/dist')
)
$ErrorActionPreference = 'Stop'
& python (Join-Path $PSScriptRoot 'install-windows.py') package --source $SourceIdentity --output $Output --store-schema $StoreSchema --release $ReleaseDirectory --extension $ExtensionDirectory
if ($LASTEXITCODE -ne 0) { throw "Package creation failed ($LASTEXITCODE)." }
