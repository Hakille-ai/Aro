# Rebuild platform icon sizes from the desktop logo, without changing its artwork.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$mobile = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$source = [Drawing.Image]::FromFile((Join-Path $mobile 'assets/logo.png'))
function Save-Icon([string]$Relative, [int]$Size) {
  $bitmap = [Drawing.Bitmap]::new($Size, $Size)
  $graphics = [Drawing.Graphics]::FromImage($bitmap)
  try {
    $graphics.Clear([Drawing.Color]::White)
    $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $edge = [int]($Size * 0.72)
    $ratio = [Math]::Min($edge / $source.Width, $edge / $source.Height)
    $width = [int]($source.Width * $ratio); $height = [int]($source.Height * $ratio)
    $graphics.DrawImage($source, [int](($Size-$width)/2), [int](($Size-$height)/2), $width, $height)
    $bitmap.Save((Join-Path $mobile $Relative), [Drawing.Imaging.ImageFormat]::Png)
  } finally { $graphics.Dispose(); $bitmap.Dispose() }
}
try {
  foreach ($entry in @{mdpi=48;hdpi=72;xhdpi=96;xxhdpi=144;xxxhdpi=192}.GetEnumerator()) {
    Save-Icon "android/app/src/main/res/mipmap-$($entry.Key)/ic_launcher.png" $entry.Value
  }
  $catalog = Get-Content (Join-Path $mobile 'ios/Runner/Assets.xcassets/AppIcon.appiconset/Contents.json') -Raw | ConvertFrom-Json
  foreach ($entry in $catalog.images) {
    $size = [double]($entry.size.Split('x')[0]) * [double]($entry.scale.TrimEnd('x'))
    Save-Icon "ios/Runner/Assets.xcassets/AppIcon.appiconset/$($entry.filename)" ([int]$size)
  }
  foreach ($size in @(192,512)) {
    Save-Icon "web/icons/Icon-$size.png" $size
    Save-Icon "web/icons/Icon-maskable-$size.png" $size
  }
  Save-Icon 'web/favicon.png' 32
} finally { $source.Dispose() }
