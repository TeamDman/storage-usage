$output = cargo build 2>&1
$succeeded = $?
if ($succeeded) {
    Write-Host "Build succeeded"
} else {
    Write-Host "Build failed"
    sd
    $code = Get-Clipboard
    $new_clipboard = $output + "`n`n" + $code
    Set-Clipboard -Value $new_clipboard
    Write-Host "Error message copied to clipboard"
}