# Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
# This file is part of the rust-photoacoustic project and is licensed under the
# SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

# This script is used to retrieve a rust_photoacoustic GET endpoint using a token.
# It requires the create_token binary to be built first.
# Usage: .\curl-auth.ps1 <url>
# Example: .\curl-auth.ps1 https://localhost:8080/api/graph-statistics

param(
    [Parameter(Mandatory=$true)]
    [string]$Url
)

# Set default values using environment variables or fallback defaults
$CreateTokenBasePath = if ($env:CREATE_TOKEN_BASE_PATH) { $env:CREATE_TOKEN_BASE_PATH } else { "./target/release/" }
$Username = if ($env:USERNAME) { $env:USERNAME } else { "admin" }
$Client = if ($env:CLIENT) { $env:CLIENT } else { "LaserSmartClient" }

# Determine the correct executable name based on platform
$CreateTokenExe = if ($IsWindows -or $env:OS -eq "Windows_NT") { 
    Join-Path $CreateTokenBasePath "create_token.exe" 
} else { 
    Join-Path $CreateTokenBasePath "create_token" 
}

# Check if the create_token executable exists
if (-not (Test-Path $CreateTokenExe)) {
    Write-Error "Error: $CreateTokenExe does not exist. Please build the project first."
    exit 1
}

try {
    # Generate the token
    $Token = & $CreateTokenExe -q --user=$Username --client=$Client
    
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to generate token"
        exit 1
    }

    # Create headers for the request
    $Headers = @{
        "Authorization" = "Bearer $Token"
    }

    # Make the HTTP request (equivalent to curl -k)
    # -SkipCertificateCheck is equivalent to curl's -k flag for ignoring SSL certificate errors
    $Response = Invoke-RestMethod -Uri $Url -Headers $Headers -SkipCertificateCheck -Method Get
    
    # Output the response
    if ($Response -is [string]) {
        Write-Output $Response
    } else {
        # If it's JSON or other structured data, convert to JSON for display
        $Response | ConvertTo-Json -Depth 10
    }
}
catch {
    Write-Error "Request failed: $($_.Exception.Message)"
    exit 1
}