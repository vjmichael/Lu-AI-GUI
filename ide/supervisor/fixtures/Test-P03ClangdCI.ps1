# P03: independent syntax gate for the exact Windows native clangd preflight.
# Fail closed before expensive OS builds; never execute the extracted commands.
[CmdletBinding()]
param(
  [string]$WorkflowPath='.github/workflows/ide-e2-native-lsp.yml'
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$lines=@(Get-Content -LiteralPath $WorkflowPath -Encoding UTF8)
$label='      - name: Prepare self-measured clangd acceptance bundle'
$starts=@(for($i=0;$i -lt $lines.Count;$i++){if($lines[$i] -ceq $label){$i}})
if($starts.Count -ne 1){throw 'P03_CLANGD_WORKFLOW_EXACT_STEP_REQUIRED'}
$at=$starts[0]
if($at+2 -ge $lines.Count -or $lines[$at+1].Trim() -ne 'shell: pwsh' -or
    $lines[$at+2].Trim() -ne 'run: |'){
  throw 'P03_CLANGD_WORKFLOW_PWSH_RUN_BLOCK_REQUIRED'
}
$body=[System.Collections.Generic.List[string]]::new()
for($i=$at+3;$i -lt $lines.Count;$i++){
  $line=$lines[$i]
  if($line -match '^      - name: '){break}
  if([string]::IsNullOrWhiteSpace($line)){$body.Add('');continue}
  if(!$line.StartsWith('          ')){
    throw 'P03_CLANGD_WORKFLOW_SCRIPT_INDENTATION_DENIED'
  }
  $body.Add($line.Substring(10))
}
if($body.Count -lt 20){throw 'P03_CLANGD_WORKFLOW_SCRIPT_MISSING'}
$tokens=$null
$errors=$null
[void][System.Management.Automation.Language.Parser]::ParseInput(
  ($body -join "`n"),[ref]$tokens,[ref]$errors)
if($errors.Count -ne 0){
  throw ('P03_CLANGD_WORKFLOW_PARSER_DENIED:'+ $errors[0].ErrorId)
}
$source=$body -join "`n"
foreach($required in @('llvm-readobj.exe','--coff-imports',
    'P03_CLANGD_DIRECT_IMPORTS_NOT_IN_PRIVATE_BUNDLE',
    'Test-Path -LiteralPath','DENIED: malformed clangd direct PE import name')){
  if(!$source.Contains($required)){throw 'P03_CLANGD_WORKFLOW_IMPORT_AUDIT_INCOMPLETE'}
}
Write-Host 'P03_CLANGD_WORKFLOW_PWSH_PARSE_AND_DIRECT_IMPORT_SCOPE=PASS'
