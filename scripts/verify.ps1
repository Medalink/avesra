param([ValidateSet('Static','Build')][string]$Suite='Static')
$ErrorActionPreference='Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
function Invoke-Checked { param([string]$Executable,[string[]]$Arguments) & $Executable @Arguments; if($LASTEXITCODE -ne 0){throw "$Executable failed with exit $LASTEXITCODE"} }
if($Suite -eq 'Static'){
 Invoke-Checked cargo @('fmt','--all','--','--check')
 Invoke-Checked cargo @('clippy','--workspace','--locked','--','-D','warnings')
 Invoke-Checked pnpm @('-r','check')
}else{
 Invoke-Checked pnpm @('-r','build')
 Invoke-Checked cargo @('build','--workspace','--release','--locked')
}
Write-Output "$Suite completed. Automated tests and live acceptance were not run."
