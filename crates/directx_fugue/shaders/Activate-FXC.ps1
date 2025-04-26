Get-Command fxc.exe > $null
if ($LASTEXITCODE -eq 0) {
    Write-Host "fxc.exe is already in the PATH."
    return
}
Write-Host "Make sure to dot-source this file!"
$env:PATH = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64;$env:PATH"