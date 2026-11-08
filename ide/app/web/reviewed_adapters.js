/**
 * DEV-03.2B: UI parser for fixed, non-executable native adapter reviews.
 * A manifest hash or a deterministic review fingerprint is NOT a grant.
 * Existing protected Cargo worker cannot consume these profile IDs.
 */
const PROJECT=/^ide-[a-f0-9]{32}$/;
const SHA=/^[a-f0-9]{64}$/;
const HEAD=/^[a-fA-F0-9]{40}$/;
const guards=[
  "reviewed_immutable_interpreter_bundle",
  "registered_snapshot_sha_verified_at_admission",
  "no_network_or_host_credentials",
  "readonly_source_private_output",
  "preexec_os_containment_and_resource_limits",
  "fresh_single_job_native_os_approval",
  "bounded_stdio_artifact_hashes",
  "descendant_kill_and_durable_recovery_receipt",
];
const step=(stepId,toolId,fixedArgs,inputOrigin)=>
  Object.freeze({stepId,toolId,fixedArgs:Object.freeze(fixedArgs),inputOrigin,
    authority:"contract_only_not_executable"});

const contracts={
  python_pytest:Object.freeze({
    manifest:"pyproject.toml",bundle:"python_pytest_bundle_unverified",
    steps:Object.freeze([
      step("pytest_fixed","reviewed_python_interpreter",
        ["-I","-m","pytest","-q","-p","no:cacheprovider"],
        "sealed_workspace_only"),
    ]),
  }),
  js_tests:Object.freeze({
    manifest:"package.json",bundle:"node_builtin_test_bundle_unverified",
    steps:Object.freeze([
      step("node_builtin_test_fixed","reviewed_node_runtime",
        ["--test","--test-reporter=tap"],"sealed_workspace_only"),
    ]),
  }),
  cpp_cmake_build:Object.freeze({
    manifest:"CMakeLists.txt",bundle:"cmake_ninja_ctest_bundle_unverified",
    steps:Object.freeze([
      step("cmake_configure_fixed","reviewed_cmake_and_ninja_bundle",
        ["-S","/workspace","-B","/output/build","-G","Ninja",
          "-DCMAKE_BUILD_TYPE=Debug"],"sealed_workspace_only"),
      step("cmake_build_fixed","reviewed_cmake_and_ninja_bundle",
        ["--build","/output/build","--parallel","1"],
        "sealed_output_from_prior_step_only"),
      step("ctest_fixed","reviewed_ctest_bundle",
        ["--test-dir","/output/build","--output-on-failure"],
        "sealed_output_from_prior_step_only"),
    ]),
  }),
};

export function planReviewedAdapterRequest(catalog,projectId,profileId) {
  const spec=contracts[profileId];
  if (!PROJECT.test(projectId) || !spec || catalog?.projectId!==projectId
      || catalog.executionAuthorized!==false || !Array.isArray(catalog.profiles))
    throw Error("DEV03_ADAPTER_NOT_REVIEWABLE");
  const profile=catalog.profiles.find(item=>item.id===profileId);
  if (!profile || profile.existingRecipe!==null
      || profile.manifest!==spec.manifest
      || profile.manifestState!=="saved_file_sha_observed_not_validated"
      || !SHA.test(profile.sha))
    throw Error("DEV03_ADAPTER_MANIFEST_NOT_REVIEWABLE");
  return Object.freeze({
    projectId,profileId,expectedSavedManifestSha256:profile.sha,
  });
}

export function checkedReviewedAdapterContract(result,request) {
  const spec=contracts[request?.profileId];
  if (!spec || !PROJECT.test(request.projectId)
      || !SHA.test(request.expectedSavedManifestSha256)
      || result?.schemaVersion!==1
      || result.projectId!==request.projectId
      || result.profileId!==request.profileId
      || result.manifestRelativePath!==spec.manifest
      || result.observedSavedManifestSha256!==request.expectedSavedManifestSha256
      || !HEAD.test(result.observedCleanGitHead)
      || !SHA.test(result.reviewFingerprint)
      || result.toolchainBundleId!==spec.bundle
      || result.reviewState!=="native_contract_observed_not_approved"
      || result.snapshotState!=="not_sealed"
      || result.toolchainState!=="not_pinned_or_installed"
      || result.nativeWorkerState!=="no_reviewed_platform_adapter"
      || result.executionAuthorized!==false
      || result.commandDispatched!==false
      || result.sourceTextExported!==false
      || result.installedAppAccepted!==false
      || !Array.isArray(result.requiredGuards)
      || result.requiredGuards.length!==guards.length
      || result.requiredGuards.some((s,i)=>s!==guards[i])
      || !Array.isArray(result.steps)
      || result.steps.length!==spec.steps.length)
    throw Error("DEV03_ADAPTER_CONTRACT_UNTRUSTED");
  const steps=result.steps.map((received,i)=>{
    const expected=spec.steps[i];
    if (!received
        || received.stepId!==expected.stepId
        || received.toolId!==expected.toolId
        || received.inputOrigin!==expected.inputOrigin
        || received.authority!=="contract_only_not_executable"
        || !Array.isArray(received.fixedArgs)
        || received.fixedArgs.length!==expected.fixedArgs.length
        || received.fixedArgs.some((arg,n)=>arg!==expected.fixedArgs[n]))
      throw Error("DEV03_ADAPTER_STEPS_UNTRUSTED");
    return expected;
  });
  return Object.freeze({
    projectId:request.projectId,
    profileId:request.profileId,
    manifest:spec.manifest,
    manifestSha:request.expectedSavedManifestSha256,
    cleanGitHead:result.observedCleanGitHead,
    fingerprint:result.reviewFingerprint,
    toolchainBundleId:spec.bundle,
    steps:Object.freeze(steps),
    guards:Object.freeze(guards),
    reviewOnly:true,
    runnable:false,
  });
}
