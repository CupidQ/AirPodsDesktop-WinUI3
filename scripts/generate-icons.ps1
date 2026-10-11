# Regenerate the application icon and manifest logos using Windows System.Drawing.
param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\app\AirPodsDesktop\Assets')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

function New-RoundedRectanglePath([float]$x, [float]$y, [float]$width, [float]$height, [float]$radius) {
    $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
    $diameter = 2 * $radius
    $path.AddArc($x, $y, $diameter, $diameter, 180, 90)
    $path.AddArc($x + $width - $diameter, $y, $diameter, $diameter, 270, 90)
    $path.AddArc($x + $width - $diameter, $y + $height - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($x, $y + $height - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return ,$path
}

$master = [System.Drawing.Bitmap]::new(1024, 1024)
$graphics = [System.Drawing.Graphics]::FromImage($master)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.ScaleTransform(4, 4)
$tile = New-RoundedRectanglePath 8 8 240 240 54
$blue = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
    [System.Drawing.Point]::new(0, 0), [System.Drawing.Point]::new(256, 256),
    [System.Drawing.Color]::FromArgb(57, 151, 255), [System.Drawing.Color]::FromArgb(32, 79, 214))
$ink = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(38, 62, 96))
$highlight = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(220, 234, 255))
try {
    $graphics.FillPath($blue, $tile)
    # Two outward-facing earbuds, with short stems and contrasting speaker vents.
    $leftStem = New-RoundedRectanglePath 81 91 27 106 13
    $rightStem = New-RoundedRectanglePath 148 91 27 106 13
    try {
        $graphics.FillPath([System.Drawing.Brushes]::White, $leftStem)
        $graphics.FillPath([System.Drawing.Brushes]::White, $rightStem)
        $graphics.FillEllipse([System.Drawing.Brushes]::White, 49, 62, 68, 65)
        $graphics.FillEllipse([System.Drawing.Brushes]::White, 139, 62, 68, 65)
        $graphics.FillEllipse($ink, 57, 78, 17, 28)
        $graphics.FillEllipse($ink, 182, 78, 17, 28)
        $graphics.FillRectangle($highlight, 88, 176, 13, 3)
        $graphics.FillRectangle($highlight, 155, 176, 13, 3)
    } finally {
        $leftStem.Dispose()
        $rightStem.Dispose()
    }
} finally {
    $graphics.Dispose()
    $tile.Dispose()
    $blue.Dispose()
    $ink.Dispose()
    $highlight.Dispose()
}

function Get-LogoPng([int]$width, [int]$height, [int]$iconSize) {
    $bitmap = [System.Drawing.Bitmap]::new($width, $height)
    $canvas = [System.Drawing.Graphics]::FromImage($bitmap)
    $stream = [System.IO.MemoryStream]::new()
    try {
        $canvas.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $canvas.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $canvas.DrawImage($master, [System.Drawing.Rectangle]::new(
            ($width - $iconSize) / 2, ($height - $iconSize) / 2, $iconSize, $iconSize))
        $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        return ,$stream.ToArray()
    } finally {
        $stream.Dispose()
        $canvas.Dispose()
        $bitmap.Dispose()
    }
}

try {
    foreach ($logo in @(
        @{ Name = 'StoreLogo'; Width = 50; Height = 50 },
        @{ Name = 'Square44x44Logo'; Width = 44; Height = 44 },
        @{ Name = 'Square150x150Logo'; Width = 150; Height = 150 },
        @{ Name = 'Wide310x150Logo'; Width = 310; Height = 150 },
        @{ Name = 'LockScreenLogo'; Width = 24; Height = 24 }
    )) {
        $bytes = Get-LogoPng $logo.Width $logo.Height ([Math]::Min($logo.Width, $logo.Height))
        [System.IO.File]::WriteAllBytes((Join-Path $OutputDirectory ($logo.Name + '.png')), $bytes)
    }

    # ICO directory followed by PNG frames; zero dimensions denote the 256px frame.
    $sizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
    $frames = @($sizes | ForEach-Object { ,(Get-LogoPng $_ $_ $_) })
    $file = [System.IO.File]::Create((Join-Path $OutputDirectory 'AppIcon.ico'))
    $writer = [System.IO.BinaryWriter]::new($file)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$sizes.Count)
        $offset = 6 + 16 * $sizes.Count
        for ($i = 0; $i -lt $sizes.Count; $i++) {
            $dimension = if ($sizes[$i] -eq 256) { 0 } else { $sizes[$i] }
            $writer.Write([byte]$dimension)
            $writer.Write([byte]$dimension)
            $writer.Write([uint16]0)
            $writer.Write([uint16]1)
            $writer.Write([uint16]32)
            $writer.Write([uint32]$frames[$i].Length)
            $writer.Write([uint32]$offset)
            $offset += $frames[$i].Length
        }
        foreach ($frame in $frames) { $writer.Write([byte[]]$frame) }
    } finally {
        $writer.Dispose()
        $file.Dispose()
    }
} finally {
    $master.Dispose()
}
