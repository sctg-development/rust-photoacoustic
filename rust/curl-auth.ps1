# Simple version using WebClient for better SSL compatibility

param(
    [Parameter(Mandatory=$true)]
    [string]$Url
)

# Set default values
$CreateTokenBasePath = if ($env:CREATE_TOKEN_BASE_PATH) { $env:CREATE_TOKEN_BASE_PATH } else { "./target/release/" }
$Username = if ($env:USER) { $env:USER } else { "admin" }
$Client = if ($env:CLIENT) { $env:CLIENT } else { "LaserSmartClient" }

# Determine the correct executable name
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

    # Configure SSL settings
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
    [System.Net.ServicePointManager]::ServerCertificateValidationCallback = { $true }

    # Create WebClient
    $WebClient = New-Object System.Net.WebClient
    $WebClient.Headers.Add("Authorization", "Bearer $Token")
      # Make the request
    Write-Host "Making request to: $Url" -ForegroundColor Yellow
    $Response = $WebClient.DownloadString($Url)
    Write-Host "Response received, length: $($Response.Length)" -ForegroundColor Green
    
    # Output the response
    if ([string]::IsNullOrEmpty($Response)) {
        Write-Host "Response is empty" -ForegroundColor Red
    } else {
        Write-Output $Response
    }
    
    $WebClient.Dispose()
}
catch {
    Write-Error "Request failed: $($_.Exception.Message)"
    if ($WebClient) {
        $WebClient.Dispose()
    }
    exit 1
}
