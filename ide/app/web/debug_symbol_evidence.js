/** DEV-04.3: read-only presentation of native executable+debug provenance.
 * Never treat symbols, a filename, or a UI object as debugger approval. */
import {safeDebugBinaryEvidence} from './debug_binary_evidence.js';
const SHA=/^[a-f0-9]{64}$/;
const REL=/^target\/debug\/[a-zA-Z0-9_+.-]{1,200}$/;
export function safeDebugSymbolEvidence(result, projectId) {
  const binary=safeDebugBinaryEvidence(result,projectId);
  const evidence=result?.debugSymbolWitness;
  if(!binary||!evidence||!REL.test(evidence.binaryRelativePath)||
     evidence.binaryRelativePath!==binary.relativePath||
     evidence.binarySha256!==binary.binarySha||
     evidence.outputTreeSha256!==binary.outputTreeSha||
     evidence.role!=='structural_binary_and_debug_information_only_no_debugger_authority'||
     evidence.debugSymbolsAttested!==true||
     evidence.sourceLineMappingVerified!==false||
     evidence.debuggerExecutionAuthorized!==false||
     evidence.installedAppAccepted!==false||
     !SHA.test(evidence.symbolsSha256))return null;
  let symbols=null;
  if(evidence.executableFormat==='elf64_x86_64_executable_dwarf_sections') {
    if(binary.format!=='elf_candidate_magic_only'||
       evidence.symbolsKind!=='embedded_dwarf_info_abbrev_line'||
       evidence.symbolsRelativePath!==null||
       evidence.validationScope!==
       'elf64_headers_load_segments_and_dwarf_unit_headers_structurally_checked_not_full_die_validation')
       return null;
  } else if(evidence.executableFormat==='pe32plus_x86_64_executable_codeview_rsds') {
    if(binary.format!=='pe_candidate_mz_only'||
       evidence.symbolsKind!=='external_pdb_guid_age_dbi_match'||
       evidence.validationScope!==
       'pe_debug_directory_rsds_guid_age_matches_pdb_info_and_dbi_not_full_symbol_parse'||
       typeof evidence.symbolsRelativePath!=='string'||
       !REL.test(evidence.symbolsRelativePath)||
       !evidence.symbolsRelativePath.endsWith('.pdb')||
       !evidence.binaryRelativePath.endsWith('.exe')||
       evidence.symbolsRelativePath!==evidence.binaryRelativePath.replace(/\.exe$/,'.pdb')||
       !Array.isArray(result.artifactFiles)||
       !result.artifactFiles.some(file=>
         file.relativePath===evidence.symbolsRelativePath &&
         file.sha256===evidence.symbolsSha256 &&
         Number.isSafeInteger(file.bytes)&&file.bytes>0))
      return null;
    symbols=evidence.symbolsRelativePath;
  } else return null;
  return Object.freeze({
    projectId,jobId:binary.jobId,planSha:binary.planSha,
    terminalReceiptSha:binary.terminalReceiptSha,
    binarySha:binary.binarySha,
    binaryPath:binary.relativePath,
    symbolsPath:symbols,
    symbolsSha:evidence.symbolsSha256,
    kind:evidence.symbolsKind,
    format:evidence.executableFormat,
    scope:evidence.validationScope,
    structuralValidation:true,
    sourceMappingVerified:false,
    debuggerAuthorized:false,
  });
}
