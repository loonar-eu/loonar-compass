
$ErrorActionPreference = 'Stop'
while ($null -ne ($line = [Console]::In.ReadLine())) {
  $m = $line | ConvertFrom-Json
  if ($null -eq $m.id) { continue }
  switch ($m.method) {
    'initialize' { $r = @{ protocolVersion = '2025-06-18'; capabilities = @{ tools = @{} }; serverInfo = @{ name = 'ps-test'; version = '1' } } }
    'tools/list' { $r = @{ tools = @(
        @{ name = 'echo'; description = 'Echoes the text'; inputSchema = @{ type = 'object'; properties = @{ text = @{ type = 'string' } } }; annotations = @{ readOnlyHint = $true } },
        @{ name = 'create ticket'; description = 'Creates a ticket'; inputSchema = @{ type = 'object'; properties = @{ title = @{ type = 'string' } } } }
      ) } }
    'tools/call' {
      if ($m.params.name -eq 'echo') { $r = @{ content = @(@{ type = 'text'; text = 'echo: ' + $m.params.arguments.text }); isError = $false } }
      else { $r = @{ content = @(@{ type = 'text'; text = 'ticket T-1 created: ' + $m.params.arguments.title }); isError = $false } }
    }
    default { $r = $null }
  }
  $out = @{ jsonrpc = '2.0'; id = $m.id; result = $r } | ConvertTo-Json -Depth 10 -Compress
  [Console]::Out.WriteLine($out); [Console]::Out.Flush()
}
