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
$Username = if ($env:USER) { $env:USER } else { "admin" }
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

    Write-Verbose "Token generated successfully" -Verbose

    # Create headers for the request
    $Headers = @{
        "Authorization" = "Bearer $Token"
    }

    Write-Verbose "Making request to: $Url" -Verbose
    Write-Verbose "PowerShell Version: $($PSVersionTable.PSVersion)" -Verbose

    # Make the HTTP request (equivalent to curl -k)
    # Use Invoke-WebRequest for better SSL compatibility
    if ($PSVersionTable.PSVersion.Major -ge 6) {
        # PowerShell 6.0+ supports -SkipCertificateCheck
        Write-Verbose "Using PowerShell 6.0+ method" -Verbose
        $WebResponse = Invoke-WebRequest -Uri $Url -Headers $Headers -SkipCertificateCheck -Method Get
        $Response = $WebResponse.Content
    } else {
        # PowerShell 5.1: More robust SSL/TLS handling
        Write-Verbose "Using PowerShell 5.1 method" -Verbose
        $OriginalCallback = [System.Net.ServicePointManager]::ServerCertificateValidationCallback
        $OriginalSecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol
        
        Write-Verbose "Original Security Protocol: $OriginalSecurityProtocol" -Verbose
        
        # Enable TLS 1.2 and disable certificate validation
        [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12 -bor [System.Net.SecurityProtocolType]::Tls11 -bor [System.Net.SecurityProtocolType]::Tls
        [System.Net.ServicePointManager]::ServerCertificateValidationCallback = { $true }
        
        Write-Verbose "New Security Protocol: $([System.Net.ServicePointManager]::SecurityProtocol)" -Verbose
        
        try {
            $WebResponse = Invoke-WebRequest -Uri $Url -Headers $Headers -Method Get
            $Response = $WebResponse.Content
        } finally {
            # Restore original settings
            [System.Net.ServicePointManager]::ServerCertificateValidationCallback = $OriginalCallback
            [System.Net.ServicePointManager]::SecurityProtocol = $OriginalSecurityProtocol
        }
    }
    
    # Output the response
    if ($Response -is [string]) {
        # Try to parse as JSON if it looks like JSON
        if ($Response.TrimStart().StartsWith('[') -or $Response.TrimStart().StartsWith('{')) {
            try {
                $JsonResponse = $Response | ConvertFrom-Json
                $JsonResponse | ConvertTo-Json -Depth 10
            } catch {
                # If JSON parsing fails, output as string
                Write-Output $Response
            }
        } else {
            Write-Output $Response
        }
    } else {
        # If it's already an object, convert to JSON for display
        $Response | ConvertTo-Json -Depth 10
    }
}
catch {
    Write-Error "Request failed: $($_.Exception.Message)"
    Write-Verbose "Full exception details: $($_.Exception)" -Verbose
    Write-Verbose "Error details: $($_.ErrorDetails)" -Verbose
    if ($_.Exception.InnerException) {
        Write-Verbose "Inner exception: $($_.Exception.InnerException.Message)" -Verbose
    }
    exit 1
}