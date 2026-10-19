param(
    [string]$Container = "beamvoice-lab-beamvoice-server-1"
)

$ErrorActionPreference = "Stop"

function Read-DotEnv([string]$Path) {
    $values = @{}
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Arquivo .env não encontrado: $Path"
    }

    foreach ($line in Get-Content -LiteralPath $Path) {
        if ($line -match '^\s*([^#=\s]+)\s*=\s*(.*?)\s*$') {
            $value = $Matches[2].Trim().Trim('"').Trim("'")
            $values[$Matches[1]] = $value
        }
    }
    return $values
}

$envFile = Join-Path $PSScriptRoot "..\.env"
$config = Read-DotEnv $envFile
$url = $config["PULSE_SIGNAL_URL"]
$room = $config["PULSE_ROOM"]
$token = $config["PULSE_AUTH_TOKEN"]

if ([string]::IsNullOrWhiteSpace($url) -or
    [string]::IsNullOrWhiteSpace($room) -or
    [string]::IsNullOrWhiteSpace($token)) {
    throw "PULSE_SIGNAL_URL, PULSE_ROOM e PULSE_AUTH_TOKEN precisam existir no .env"
}

$uri = [Uri]$url
$containerState = docker inspect --format '{{.State.Status}}' $Container 2>$null
if (-not $containerState) {
    throw "Container não encontrado: $Container"
}

$containerIp = docker inspect --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' $Container
$publishedPorts = docker inspect --format '{{json .NetworkSettings.Ports}}' $Container
$publishedPort = if ($publishedPorts -and $publishedPorts -notmatch 'null' -and $publishedPorts -match ('"' + $uri.Port + '/')) {
    $publishedPorts
} else {
    $null
}
$tcpReachable = Test-NetConnection -ComputerName $uri.Host -Port $uri.Port -InformationLevel Quiet

Write-Host "PULSE_SIGNAL_URL : $url"
Write-Host "PULSE_ROOM       : $room"
Write-Host "TOKEN            : presente ($($token.Length) caracteres)"
Write-Host "CONTAINER        : $Container ($containerState)"
Write-Host "CONTAINER_IP     : $containerIp"
Write-Host "PUBLISHED_PORT   : $(if ($publishedPort) { $publishedPort } else { '<nenhuma>' })"
Write-Host "TCP_ENDPOINT     : $(if ($tcpReachable) { 'acessível' } else { 'inacessível' })"

if ($uri.Host -eq "127.0.0.1" -and -not $publishedPort) {
    Write-Warning "A URL aponta para localhost, mas o container não publica a porta $($uri.Port)."
}
if ($uri.Host -eq $containerIp) {
    Write-Warning "A URL usa o IP interno do container; prefira uma porta publicada no host."
}
if ($containerState -eq "running" -and $containerIp -and -not $publishedPort) {
    Write-Warning "O container está ativo, mas não há porta publicada e o processo informado anteriormente é apenas sleep."
}

if (-not $tcpReachable) {
    exit 2
}

try {
    $health = Invoke-WebRequest -Uri "$($uri.Scheme.Replace('ws','http'))://$($uri.Host):$($uri.Port)/health" -UseBasicParsing
    Write-Host "HEALTH           : HTTP $($health.StatusCode)"
} catch {
    Write-Warning "TCP respondeu, mas /health falhou: $($_.Exception.Message)"
    exit 3
}

$socket = [System.Net.WebSockets.ClientWebSocket]::new()
$timeout = [Threading.CancellationTokenSource]::new([TimeSpan]::FromSeconds(5))
try {
    $socket.ConnectAsync($uri, $timeout.Token).GetAwaiter().GetResult() | Out-Null
    foreach ($message in @(
        @{ type = "auth"; token = $token },
        @{ type = "join"; room = $room }
    )) {
        $bytes = [Text.Encoding]::UTF8.GetBytes(($message | ConvertTo-Json -Compress))
        $socket.SendAsync([ArraySegment[byte]]::new($bytes),
            [System.Net.WebSockets.WebSocketMessageType]::Text, $true,
            $timeout.Token).GetAwaiter().GetResult() | Out-Null
        $buffer = [byte[]]::new(20000)
        $reply = $socket.ReceiveAsync([ArraySegment[byte]]::new($buffer),
            $timeout.Token).GetAwaiter().GetResult()
        $response = [Text.Encoding]::UTF8.GetString($buffer, 0, $reply.Count) | ConvertFrom-Json
        if ($response.type -eq "error") { throw "Servidor rejeitou $($message.type): $($response.reason)" }
        if ($message.type -eq "auth" -and $response.type -ne "authenticated") { throw "Resposta inesperada à autenticação: $($response.type)" }
        if ($message.type -eq "join" -and $response.type -ne "joined") { throw "Resposta inesperada ao ingresso: $($response.type)" }
        Write-Host "$($message.type.ToUpper())            : OK"
    }
} catch {
    Write-Warning "WebSocket/auth/join falhou: $($_.Exception.Message)"
    exit 4
} finally {
    $socket.Dispose()
    $timeout.Dispose()
}
