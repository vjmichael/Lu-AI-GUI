# External HOST-POSITIVE fixed dual-stack loopback control, not WFP proof.
# This native elevated test companion listens only on 127.0.0.1 and ::1
# port 49281. Execute before starting actual approved target + pinned LLDB;
# run the independent Test-FIN04W2-EffectiveWfpNative.ps1 while it remains
# active; stop with Ctrl+C. No WFP rule, service or authorization is created.
[CmdletBinding()]
param([Parameter(Mandatory)][switch]$ExecuteHostLoopbackControl)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
if(!$ExecuteHostLoopbackControl -or $env:OS -ne 'Windows_NT') {
  throw 'FIN04W2_CONTROL_DENIED:original_Windows_operator_required'
}
$ipv4=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,49281)
$ipv6=[Net.Sockets.TcpListener]::new([Net.IPAddress]::IPv6Loopback,49281)
try {
  $ipv4.Start()
  $ipv6.Server.DualMode=$false
  $ipv6.Start()
  Write-Host 'FIN04W2_HOST_LOOPBACK_CONTROLS_LISTENING=49281;NO_WFP_PROOF'
  while($true) {
    foreach($listener in @($ipv4,$ipv6)) {
      if($listener.Pending()) {
        $client=$listener.AcceptTcpClient()
        $client.Dispose()
      }
    }
    Start-Sleep -Milliseconds 30
  }
} finally {
  $ipv4.Stop()
  $ipv6.Stop()
}
