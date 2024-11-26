# build-and-fix.ps1

# Run cargo build and capture the output
$cargo_output = cargo build --example exploration --message-format json

# Initialize a hashtable to store suggestions per file
$suggestions_per_file = @{}

foreach ($line in $cargo_output) {
    try {
        $msg = $line | ConvertFrom-Json
    } catch {
        continue
    }

    if ($msg.reason -eq 'compiler-message') {
        $message = $msg.message

        # Process the main message spans
        foreach ($span in $message.spans) {
            if ($span.suggested_replacement) {
                $file_name = $span.file_name
                if (-not $suggestions_per_file.ContainsKey($file_name)) {
                    $suggestions_per_file[$file_name] = @()
                }

                $suggestions_per_file[$file_name] += $span
            }
        }

        # Process the children messages
        foreach ($child in $message.children) {
            foreach ($span in $child.spans) {
                if ($span.suggested_replacement) {
                    $file_name = $span.file_name
                    if (-not $suggestions_per_file.ContainsKey($file_name)) {
                        $suggestions_per_file[$file_name] = @()
                    }

                    $suggestions_per_file[$file_name] += $span
                }
            }
        }
    }
}

# Now, for each file, apply the suggestions
foreach ($file_name in $suggestions_per_file.Keys) {
    $spans = $suggestions_per_file[$file_name]

    # Sort spans by byte_start in descending order to avoid position shifts
    $sorted_spans = $spans | Sort-Object -Property byte_start -Descending

    # Read the file content
    $file_content = Get-Content $file_name -Raw

    # Convert the content to bytes
    $file_bytes = [System.Text.Encoding]::UTF8.GetBytes($file_content)
    $file_length = $file_bytes.Length

    # Create a list to store the byte ranges to be replaced
    $replacements = @()

    foreach ($span in $sorted_spans) {
        $byte_start = $span.byte_start
        $byte_end = $span.byte_end
        $suggested_replacement = $span.suggested_replacement

        $replacements += [PSCustomObject]@{
            ByteStart = $byte_start
            ByteEnd = $byte_end
            ReplacementBytes = [System.Text.Encoding]::UTF8.GetBytes($suggested_replacement)
        }
    }

    # Apply the replacements
    $new_file_bytes = $file_bytes

    foreach ($replacement in $replacements) {
        $byte_start = $replacement.ByteStart
        $byte_end = $replacement.ByteEnd
        $replacement_bytes = $replacement.ReplacementBytes

        # Remove the original bytes and insert the replacement
        $before = $new_file_bytes[0..($byte_start - 1)]
        $after = $new_file_bytes[$byte_end..($new_file_bytes.Length - 1)]
        $new_file_bytes = $before + $replacement_bytes + $after
    }

    # Convert the bytes back to string
    $new_content = [System.Text.Encoding]::UTF8.GetString($new_file_bytes)

    # Write the new content back to the file
    Set-Content $file_name $new_content -Encoding UTF8
}
