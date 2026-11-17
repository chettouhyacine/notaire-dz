# Copies docs/manual/img/*.png to docs/screenshots/*.png, cropping the narrow strip on the left
# that holds the small overlay badge drawn by the capture window, so published images show only the
# application window.
Add-Type -AssemblyName System.Drawing

$source = 'F:\NotaireDZ\docs\manual\img'
$destination = 'F:\NotaireDZ\docs\screenshots'
New-Item -ItemType Directory -Force -Path $destination | Out-Null

$map = @{
    'shot-01-welcome'    = 'welcome'
    'shot-02-login'      = 'login'
    'shot-03-recovery'   = 'recovery-code'
    'shot-04-onboarding' = 'onboarding'
    'shot-05-home'       = 'home'
    'shot-06-clients'    = 'clients'
    'shot-07-cases'      = 'cases'
    'shot-08-settings'   = 'settings'
}

foreach ($key in $map.Keys) {
    $path = Join-Path $source "$key.png"
    $bmp = [System.Drawing.Bitmap]::FromFile($path)
    $width = $bmp.Width
    $height = $bmp.Height

    # The badge text overflows the crop, so the corner is first flattened to the bar colour.
    $colour = $bmp.GetPixel([int]($width * 0.195), [int]($height * 0.008))
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $brush = New-Object System.Drawing.SolidBrush $colour
    $graphics.FillRectangle($brush, 0, 0, [int]($width * 0.094), [int]($height * 0.054))
    $graphics.Dispose()
    $brush.Dispose()

    $cropLeft = [int]($width * 0.079)
    $rect = New-Object System.Drawing.Rectangle $cropLeft, 0, ($width - $cropLeft), $height
    $copy = $bmp.Clone($rect, $bmp.PixelFormat)

    $out = Join-Path $destination ("$($map[$key]).png")
    $copy.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $copy.Dispose()
    $bmp.Dispose()
    Write-Output ("$key -> $($map[$key]).png ${width}x${height} cropped ${cropLeft}px corner=$colour")
}
