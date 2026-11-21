/**
 * DEV-03.1: closed, display-only host-observed project build/test catalog.
 * A profile selection may choose an existing WO6 Cargo recipe, but cannot
 * request a process launch, change a manifest path or grant execution.
 */
const definitions = [
  ["rust_check", "rust", "check", "Cargo.toml", "rust_check"],
  ["rust_tests", "rust", "test", "Cargo.toml", "rust_tests"],
  ["python_pytest", "python", "test", "pyproject.toml", null],
  ["js_tests", "javascript", "test", "package.json", null],
  ["cpp_cmake_build", "cpp", "build", "CMakeLists.txt", null],
  ["lu_ide_host_tests", "rust", "test", "ide/host/Cargo.toml", "lu_ide_host_tests"],
  ["rust_build", "rust", "build", "Cargo.toml", "rust_build"],
];
const byId=new Map(definitions.map(([id, language, operation, path, recipe])=>
  [id,{language,operation,path,recipe}]));
const SHA=/^[a-f0-9]{64}$/;
const PROJECT=/^ide-[a-f0-9]{32}$/;
const states=new Set(["saved_file_sha_observed_not_validated","identity_mismatch",
  "not_found","not_readable_or_unverifiable"]);
const scans=new Set(["not_test_profile","partial_conventional_tests_only",
  "bounded_conventional_tests_only_not_discovery_complete",
  "native_identity_mismatch","tests_directory_absent_not_no_tests",
  "tests_directory_unverifiable"]);
export function safeProjectRunProfileDiscovery(result,projectId) {
  if (!PROJECT.test(projectId) || result?.schemaVersion!==1 ||
      result?.projectId!==projectId ||
      result.source!=="fixed_host_manifest_and_conventional_tests_inspection" ||
      result.manifestContentsExported!==false || result.processStarted!==false ||
      result.workerGrantIssued!==false || result.taskAccepted!==false ||
      !Array.isArray(result.profiles) || result.profiles.length!==definitions.length)
    throw Error("DEV03_DISCOVERY_SHAPE_DENIED");
  const seen=new Set();
  const profiles=result.profiles.map(raw=>{
    const spec=byId.get(raw?.profileId);
    if (!spec || seen.has(raw.profileId) ||
        raw.language!==spec.language || raw.operation!==spec.operation ||
        raw.manifestRelativePath!==spec.path ||
        raw.existingNativeRecipeId!==spec.recipe ||
        raw.executionRoute!==(spec.recipe
          ? "existing_wo6_protected_cargo_requires_fresh_authorization"
          : "not_implemented_requires_reviewed_native_adapter") ||
        raw.executionAuthorized!==false || !states.has(raw.manifestState) ||
        !(raw.manifestSha256===null ||
          SHA.test(raw.manifestSha256)) ||
        (raw.manifestState==="saved_file_sha_observed_not_validated")!==
          (typeof raw.manifestSha256==="string") ||
        typeof raw.title!=="string" || raw.title.length>120 ||
        !Number.isSafeInteger(raw.testCandidates) || raw.testCandidates<0 ||
        raw.testCandidates>400 || !scans.has(raw.testScanState) ||
        !Array.isArray(raw.testHintPaths) || raw.testHintPaths.length>20 ||
        !raw.testHintPaths.every(path=>typeof path==="string" &&
          path.startsWith("tests/") && path.length<=1024 &&
          !path.includes("\\") && !/[\x00-\x1f\x7f]/.test(path) &&
          path.split("/").every(segment=>segment && segment!=="." &&
            segment!==".." && !segment.includes(":"))))
      throw Error("DEV03_PROFILE_UNTRUSTED");
    seen.add(raw.profileId);
    return Object.freeze({
      id:raw.profileId,title:raw.title,language:raw.language,
      operation:raw.operation,manifest:spec.path,manifestState:raw.manifestState,
      sha:raw.manifestSha256,existingRecipe:spec.recipe,
      testCandidates:raw.testCandidates,hints:Object.freeze([...raw.testHintPaths]),
      scanState:raw.testScanState,
    });
  });
  if (seen.size!==definitions.length) throw Error("DEV03_PROFILE_INCOMPLETE");
  return Object.freeze({projectId,profiles:Object.freeze(profiles),
    executionAuthorized:false});
}
/** This is selection of a known recipe string only, never a worker grant. */
export function selectExistingProtectedRecipe(catalog,profileId,projectId) {
  if (!catalog || catalog.projectId!==projectId || !PROJECT.test(projectId))
    throw Error("DEV03_PROJECT_STALE");
  const profile=catalog.profiles.find(p=>p.id===profileId);
  if (!profile || !profile.existingRecipe ||
      profile.existingRecipe!==profile.id ||
      profile.manifestState!=="saved_file_sha_observed_not_validated" ||
      !SHA.test(profile.sha))
    throw Error("DEV03_EXECUTION_ADAPTER_UNAVAILABLE");
  return profile.existingRecipe;
}
