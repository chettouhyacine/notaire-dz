param([Parameter(Mandatory = $true)][string]$Target)

# Desktop + Start Menu shortcuts for the installed copy.
$exe = Join-Path $Target 'NotaireDZ.exe'
$ico = Join-Path $Target 'NotaireDZ.ico'
if (-not (Test-Path $exe)) {
    Write-Output "ERROR: NotaireDZ.exe not found in $Target"
    exit 1
}

$ws = New-Object -ComObject WScript.Shell
$desktop = [Environment]::GetFolderPath('Desktop')
$programs = [Environment]::GetFolderPath('Programs')
$startDir = Join-Path $programs 'NotaireDZ'
if (-not (Test-Path $startDir)) { New-Item -ItemType Directory -Path $startDir -Force | Out-Null }

function New-Shortcut([string]$Path, [string]$Arguments) {
    $sc = $ws.CreateShortcut($Path)
    $sc.TargetPath = $exe
    $sc.Arguments = $Arguments
    $sc.WorkingDirectory = $Target
    $sc.IconLocation = "$ico,0"
    $sc.Description = 'NotaireDZ - office notarial (hors ligne, chiffre)'
    $sc.WindowStyle = 1
    $sc.Save()
    Write-Output "shortcut: $Path"
}

New-Shortcut (Join-Path $desktop 'NotaireDZ.lnk') ''
New-Shortcut (Join-Path $startDir 'NotaireDZ.lnk') ''
New-Shortcut (Join-Path $desktop 'NotaireDZ (demo).lnk') '--demo'
New-Shortcut (Join-Path $startDir 'NotaireDZ (demo).lnk') '--demo'
