// MEMORY-LOOP-03E: read-only operator labels, never tool dispatch.
export function explainMemoryProjectionRecovery(entry){
  switch(entry?.status){
    case 'RECONSTRUCTED_SOURCE_VERIFIED':
      return {title:'Historical memory projection reconstructed',
        description:'The old effect and original source were verified at observation. No current source or new authorization is asserted.'};
    case 'LEGACY_REVIEW_REQUIRED':
      return {title:'Legacy memory evidence needs human review',
        description:'No atomic projection plan exists. Lu did not invent or retry one.'};
    default:
      return {title:'Memory reconstruction requires human review',
        description:'Original sources, corrections or projection records prevent safe automatic publication.'};
  }
}
