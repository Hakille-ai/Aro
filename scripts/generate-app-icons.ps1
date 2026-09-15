param(
  [string]$Source = "apps/desktop/public/logo.png",
  [string]$OutputDir = "apps/desktop/src-tauri/icons",
  [string]$PublicIcon = "apps/desktop/public/app-icon.png"
)

Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"

function New-RoundedRectPath {
  param(
    [float]$X,
    [float]$Y,
    [float]$Width,
    [float]$Height,
    [float]$Radius
  )

  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $diameter = $Radius * 2
  $path.AddArc($X, $Y, $diameter, $diameter, 180, 90)
  $path.AddArc($X + $Width - $diameter, $Y, $diameter, $diameter, 270, 90)
  $path.AddArc($X + $Width - $diameter, $Y + $Height - $diameter, $diameter, $diameter, 0, 90)
  $path.AddArc($X, $Y + $Height - $diameter, $diameter, $diameter, 90, 90)
  $path.CloseFigure()
  return $path
}

function Save-PngBytes {
  param(
    [System.Drawing.Bitmap]$Image
  )

  $stream = New-Object System.IO.MemoryStream
  $Image.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
  $bytes = $stream.ToArray()
  $stream.Dispose()
  return $bytes
}

function Get-IcoDibBytes {
  param(
    [System.Drawing.Bitmap]$Image
  )

  $width = $Image.Width
  $height = $Image.Height
  $pixelStride = $width * 4
  $maskStride = [int]([Math]::Floor(($width + 31) / 32) * 4)
  $pixelBytes = $pixelStride * $height
  $maskBytes = $maskStride * $height

  $stream = New-Object System.IO.MemoryStream
  $writer = New-Object System.IO.BinaryWriter($stream)
  try {
    $writer.Write([UInt32]40)
    $writer.Write([Int32]$width)
    $writer.Write([Int32]($height * 2))
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]32)
    $writer.Write([UInt32]0)
    $writer.Write([UInt32]$pixelBytes)
    $writer.Write([Int32]0)
    $writer.Write([Int32]0)
    $writer.Write([UInt32]0)
    $writer.Write([UInt32]0)

    for ($y = $height - 1; $y -ge 0; $y--) {
      for ($x = 0; $x -lt $width; $x++) {
        $pixel = $Image.GetPixel($x, $y)
        $writer.Write([byte]$pixel.B)
        $writer.Write([byte]$pixel.G)
        $writer.Write([byte]$pixel.R)
        $writer.Write([byte]$pixel.A)
      }
    }

    for ($i = 0; $i -lt $maskBytes; $i++) {
      $writer.Write([byte]0)
    }

    return $stream.ToArray()
  } finally {
    $writer.Dispose()
    $stream.Dispose()
  }
}

function Write-Ico {
  param(
    [string]$Path,
    [byte[][]]$PngEntries,
    [int[]]$Sizes
  )

  $stream = [System.IO.File]::Create($Path)
  $writer = New-Object System.IO.BinaryWriter($stream)
  try {
    $writer.Write([UInt16]0)
    $writer.Write([UInt16]1)
    $writer.Write([UInt16]$PngEntries.Length)

    $offset = 6 + ($PngEntries.Length * 16)
    for ($index = 0; $index -lt $PngEntries.Length; $index++) {
      $size = $Sizes[$index]
      $bytes = $PngEntries[$index]
      $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
      $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
      $writer.Write([byte]0)
      $writer.Write([byte]0)
      $writer.Write([UInt16]1)
      $writer.Write([UInt16]32)
      $writer.Write([UInt32]$bytes.Length)
      $writer.Write([UInt32]$offset)
      $offset += $bytes.Length
    }

    foreach ($bytes in $PngEntries) {
      $writer.Write($bytes)
    }
  } finally {
    $writer.Dispose()
    $stream.Dispose()
  }
}

function New-AppIcon {
  param(
    [System.Drawing.Bitmap]$SourceLogo,
    [int]$Size
  )

  $minX = $SourceLogo.Width
  $minY = $SourceLogo.Height
  $maxX = 0
  $maxY = 0

  for ($y = 0; $y -lt $SourceLogo.Height; $y++) {
    for ($x = 0; $x -lt $SourceLogo.Width; $x++) {
      $pixel = $SourceLogo.GetPixel($x, $y)
      $isBackground = $pixel.A -lt 8 -or ($pixel.R -gt 246 -and $pixel.G -gt 246 -and $pixel.B -gt 246)
      if (-not $isBackground) {
        if ($x -lt $minX) { $minX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }

  $logoWidth = [Math]::Max(1, $maxX - $minX + 1)
  $logoHeight = [Math]::Max(1, $maxY - $minY + 1)
  $trimmedLogo = New-Object System.Drawing.Bitmap($logoWidth, $logoHeight, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  for ($y = 0; $y -lt $logoHeight; $y++) {
    for ($x = 0; $x -lt $logoWidth; $x++) {
      $pixel = $SourceLogo.GetPixel($minX + $x, $minY + $y)
      if ($pixel.R -gt 246 -and $pixel.G -gt 246 -and $pixel.B -gt 246) {
        $trimmedLogo.SetPixel($x, $y, [System.Drawing.Color]::FromArgb(0, 255, 255, 255))
      } else {
        $trimmedLogo.SetPixel($x, $y, $pixel)
      }
    }
  }

  $canvas = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $graphics = [System.Drawing.Graphics]::FromImage($canvas)
  $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $graphics.Clear([System.Drawing.Color]::Transparent)

  $isTiny = $Size -le 32
  $isSmall = $Size -le 64
  [single]$marginRatio = if ($isTiny) { 0.085 } elseif ($isSmall) { 0.078 } elseif ($Size -le 128) { 0.07 } else { 0.066 }
  [single]$margin = [single][Math]::Round($Size * $marginRatio)
  [single]$iconSize = [single]($Size - ($margin * 2))
  $iconRect = New-Object System.Drawing.RectangleF -ArgumentList $margin, $margin, $iconSize, $iconSize
  $radius = $Size * 0.215

  if (-not $isSmall) {
    for ($i = 7; $i -ge 1; $i--) {
      $shadowOffset = $Size * (0.0048 * $i)
      $shadowInset = $Size * (0.004 * $i)
      $shadowPath = New-RoundedRectPath `
        ($iconRect.X + $shadowInset) `
        ($iconRect.Y + $shadowOffset + $shadowInset) `
        ($iconRect.Width - ($shadowInset * 2)) `
        ($iconRect.Height - ($shadowInset * 2)) `
        $radius
      $shadowBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb([Math]::Max(5, 32 - ($i * 3)), 28, 47, 64))
      $graphics.FillPath($shadowBrush, $shadowPath)
      $shadowBrush.Dispose()
      $shadowPath.Dispose()
    }
  }

  $backgroundPath = New-RoundedRectPath $iconRect.X $iconRect.Y $iconRect.Width $iconRect.Height $radius
  $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    $iconRect,
    [System.Drawing.Color]::FromArgb(255, 253, 255, 252),
    [System.Drawing.Color]::FromArgb(255, 226, 241, 255),
    [System.Drawing.Drawing2D.LinearGradientMode]::ForwardDiagonal
  )
  $graphics.FillPath($gradient, $backgroundPath)
  $gradient.Dispose()

  if (-not $isSmall) {
    $accentGreen = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(22, 46, 211, 91))
    $accentBlue = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(24, 10, 132, 255))
    $accentGold = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(16, 255, 204, 48))
    $graphics.FillEllipse($accentGreen, $Size * 0.08, $Size * 0.08, $Size * 0.34, $Size * 0.25)
    $graphics.FillEllipse($accentBlue, $Size * 0.56, $Size * 0.62, $Size * 0.30, $Size * 0.22)
    $graphics.FillEllipse($accentGold, $Size * 0.66, $Size * 0.13, $Size * 0.20, $Size * 0.16)
    $accentGreen.Dispose()
    $accentBlue.Dispose()
    $accentGold.Dispose()
  }

  $borderPen = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb($(if ($isSmall) { 178 } else { 116 }), 255, 255, 255), [Math]::Max(1, $Size * 0.007))
  $graphics.DrawPath($borderPen, $backgroundPath)
  $borderPen.Dispose()

  $widthRatio = if ($isTiny) { 0.70 } elseif ($isSmall) { 0.72 } elseif ($Size -le 128) { 0.70 } else { 0.68 }
  $heightRatio = if ($isTiny) { 0.76 } elseif ($isSmall) { 0.78 } elseif ($Size -le 128) { 0.78 } else { 0.76 }
  $scale = [Math]::Min(($Size * $widthRatio) / $logoWidth, ($Size * $heightRatio) / $logoHeight)
  $drawWidth = $logoWidth * $scale
  $drawHeight = $logoHeight * $scale
  $drawX = ($Size - $drawWidth) / 2
  $drawY = ($Size - $drawHeight) / 2 + ($Size * $(if ($isSmall) { 0.006 } else { 0.012 }))
  if ($isSmall) {
    $drawWidth = [Math]::Round($drawWidth)
    $drawHeight = [Math]::Round($drawHeight)
    $drawX = [Math]::Round(($Size - $drawWidth) / 2)
    $drawY = [Math]::Round(($Size - $drawHeight) / 2)
  }
  $logoRect = New-Object System.Drawing.RectangleF($drawX, $drawY, $drawWidth, $drawHeight)

  $graphics.DrawImage($trimmedLogo, $logoRect)
  $backgroundPath.Dispose()
  $graphics.Dispose()
  $trimmedLogo.Dispose()

  return $canvas
}

$sourcePath = (Resolve-Path $Source).Path
$outputPath = (Resolve-Path $OutputDir).Path
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$sourceLogo = [System.Drawing.Bitmap]::FromFile($sourcePath)
$icon1024 = New-AppIcon -SourceLogo $sourceLogo -Size 1024
$icon1024.Save((Join-Path $outputPath "icon.png"), [System.Drawing.Imaging.ImageFormat]::Png)

$publicIconPath = Join-Path (Resolve-Path (Split-Path $PublicIcon -Parent)).Path (Split-Path $PublicIcon -Leaf)
$icon1024.Save($publicIconPath, [System.Drawing.Imaging.ImageFormat]::Png)

$icoSizes = @(32, 16, 20, 24, 30, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256)
$entries = New-Object "System.Collections.Generic.List[byte[]]"
foreach ($size in $icoSizes) {
  $icon = New-AppIcon -SourceLogo $sourceLogo -Size $size
  $entries.Add((Get-IcoDibBytes -Image $icon))
  $icon.Dispose()
}
Write-Ico -Path (Join-Path $outputPath "icon.ico") -PngEntries $entries.ToArray() -Sizes $icoSizes

$icon1024.Dispose()
$sourceLogo.Dispose()

Write-Host "Generated app icons from $sourcePath"
