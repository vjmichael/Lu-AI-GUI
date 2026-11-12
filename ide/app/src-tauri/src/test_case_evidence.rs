//! INT03-E: original-job, bounded **reported** test-case observations.
 //!
 //! Native process supervision/terminal receipts prove the job identity and
 //! kernel custody, NOT the truth of project-controlled stdout. This module
 //! interprets only fixed runner formats, anchors optional source coordinates
 //! to independently hashed frozen AND currently saved project bytes, and
 //! never grants execution, task acceptance or independent test assertion proof.
 use lu_ide_execution_supervisor::{
     live_output::OutputPage,
     snapshot::{verify_input, PublishedInput},
 };
 use lu_ide_workspace::IdeWorkspaceRegistry;
 use serde::Serialize;
 use sha2::{Digest, Sha256};
 use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Component, Path}};

 const MAX_CASES: usize = 128;
 const MAX_LINE: usize = 1024;
 const MAX_SOURCE_BYTES: u64 = 512 * 1024;

 #[derive(Clone, Debug, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct SourceAnchor {
     pub relative_path: String,
     pub disk_sha256: String,
     /// Source 1-based line, verified within frozen file's UTF-8 text.
     /// This location is framework-reported, not independently fault-proven.
     pub line: u32,
     pub character_utf16: u32,
 }

 #[derive(Clone, Debug, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct TestCaseEvidence {
     pub case_id: String,
     pub name: String,
     pub status: String,
     pub reporter: &'static str,
     pub source: Option<SourceAnchor>,
     /// Always false until a NEW native source review and exact editor check.
     pub navigation_verified: bool,
     pub assertion_independently_verified: bool,
 }

 #[derive(Clone, Debug, Serialize)]
 #[serde(rename_all = "camelCase")]
 pub struct TestCaseReport {
     pub schema_version: u32,
     pub project_id: String,
     pub job_id: String,
     pub recipe_id: String,
     pub plan_sha256: String,
     pub terminal_receipt_sha256: String,
     pub source_head: String,
     pub input_snapshot_sha256: String,
     pub manifest_sha256: String,
     pub toolchain_sha256: String,
     /// Digest of the bounded, ephemeral observed output window only.
     pub observed_log_sha256: String,
     pub capture_truncated: bool,
     pub cases_truncated: bool,
     pub case_count_observed: usize,
     pub cases: Vec<TestCaseEvidence>,
     pub evidence_state: &'static str,
     pub assertion_independently_verified: bool,
     pub accepted_by_human: bool,
     pub installed_app_accepted: bool,
 }

 #[derive(Clone, Debug)]
 struct Candidate {
     name: String,
     status: &'static str,
     reporter: &'static str,
     /// Optional test-source filename reported by the runner, not trusted.
     path: Option<String>,
 }

 fn safe_path(path: &str) -> bool {
     path.len() <= 512
         && !path.is_empty()
         && !path.starts_with('/')
         && !path.contains('\\')
         && !path.contains(':')
         && !path.chars().any(char::is_control)
         && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
         && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
 }

 fn reported_pytest_location(line: &str) -> Option<(String, u32)> {
     // Strict saved-project relative traceback line only; absolute file URIs
     // and adapter-provided parent traversal paths are never capabilities.
     let (path, rest) = line.split_once(':')?;
     if !path.ends_with(".py") || !safe_path(path) {
         return None;
     }
     let digits = rest.split_once(':')?.0;
     if digits.is_empty() || digits.len() > 6 ||
         !digits.bytes().all(|b| b.is_ascii_digit()) {
         return None;
     }
     let line_no = digits.parse::<u32>().ok()?;
     if line_no == 0 || line_no > 250_000 { return None; }
     Some((path.to_owned(), line_no))
 }

 fn parse_candidate(recipe: &str, line: &str) -> Option<Candidate> {
     let row = line.trim();
     let (reporter, name, status, path) = match recipe {
         "rust_tests" | "lu_ide_host_tests" => {
             let suffix = row.strip_prefix("test ")?;
             let (name, status) = suffix.rsplit_once(" ... ")?;
             let status = match status.trim() {
                 "ok" => "reported_pass",
                 "FAILED" => "reported_fail",
                 "ignored" => "reported_skip",
                 _ => return None,
             };
             ("cargo_libtest", name, status, None)
         }
         id if lu_ide_execution_supervisor::rust_exact_case_name(id).is_some()=>{
             let suffix=row.strip_prefix("test ")?;
             let (name,status)=suffix.rsplit_once(" ... ")?;
             let status=match status.trim(){
               "ok"=>"reported_pass","FAILED"=>"reported_fail",
               "ignored"=>"reported_skip",_=>return None,
             };
             // A completed exit-0 with no matching case output cannot
             // become a reported passing test.
             let expected=lu_ide_execution_supervisor::rust_exact_case_name(id)?;
             if name!=expected{return None;}
             ("cargo_libtest_exact_filter",name,status,None)
         }
         "python_pytest" => {
             let (status, tail) = if let Some(value) = row.strip_prefix("FAILED ") {
                 ("reported_fail", value)
             } else if let Some(value) = row.strip_prefix("PASSED ") {
                 ("reported_pass", value)
             } else if let Some(value) = row.strip_prefix("SKIPPED ") {
                 ("reported_skip", value)
             } else {
                 return None;
             };
             let token = tail.split_whitespace().next()?;
             let path = token.split("::").next()?;
             if !path.ends_with(".py") || !safe_path(path) { return None; }
             ("pytest_reported", token, status, Some(path.to_owned()))
         }
         "js_tests" => {
             let (status, tail) = if let Some(value) = row.strip_prefix("not ok ") {
                 ("reported_fail", value)
             } else if let Some(value) = row.strip_prefix("ok ") {
                 ("reported_pass", value)
             } else { return None };
             let (number, name) = tail.split_once(" - ")?;
             if number.len() > 6 || number.is_empty() ||
                 !number.bytes().all(|b| b.is_ascii_digit()) ||
                 number.parse::<u32>().ok()? == 0 {
                 return None;
             }
             ("node_tap_reported", name, status, None)
         }
         "cpp_cmake_build" => {
             let (_, tail) = row.split_once("Test #")?;
             let (number, label) = tail.split_once(':')?;
             if number.trim().is_empty() || number.trim().len()>6 ||
                 !number.trim().bytes().all(|b| b.is_ascii_digit()) {
                 return None;
             }
             let label = label.trim();
             let (name, status) = if let Some((name, _)) = label.rsplit_once(" ***Failed") {
                 (name.trim_end_matches('.').trim(), "reported_fail")
             } else if let Some((name, _)) = label.rsplit_once(" Passed") {
                 (name.trim_end_matches('.').trim(), "reported_pass")
             } else if let Some((name, _)) = label.rsplit_once(" ***Not Run") {
                 (name.trim_end_matches('.').trim(), "reported_skip")
             } else { return None };
             ("ctest_reported", name, status, None)
         }
         _ => return None,
     };
     if name.is_empty() || name.len() > 240 ||
         name.chars().any(char::is_control) {
         return None;
     }
     Some(Candidate {name:name.to_owned(),status,reporter,path})
 }

 fn capture_lines(capture: &OutputPage) -> (Vec<String>, String, bool) {
     let mut hash = Sha256::new();
     let mut partial = capture.truncated || capture.chunks.len() > 256;
     let mut last_sequence = 0;
     let mut streams = BTreeMap::<String,String>::new();
     let mut lines = Vec::new();
     for event in capture.chunks.iter().take(256) {
         if !matches!(event.stream, "stdout" | "stderr" | "combined") ||
             event.sequence <= last_sequence || event.text.len()>2048 {
             partial = true;
             continue;
         }
         last_sequence = event.sequence;
         hash.update(event.sequence.to_le_bytes());
         hash.update(event.stream.as_bytes());
         hash.update([0]);
         hash.update(event.text.as_bytes());
         hash.update([0]);
         let buffer = streams.entry(event.stream.to_owned()).or_default();
         buffer.push_str(&event.text);
         while let Some(index) = buffer.find('\n') {
             let line: String = buffer.drain(..=index).collect();
             if line.len() > MAX_LINE {partial=true;} else {lines.push(line);}
         }
         if buffer.len() > MAX_LINE {
             partial = true;
             buffer.clear();
         }
     }
     // Do not promote a truncated final line into a complete test outcome.
     if streams.values().any(|part| !part.is_empty()) { partial=true; }
     (lines,format!("{:x}",hash.finalize()),partial)
 }

 fn sealed_source_anchor(
     input: &PublishedInput,
     registry: &IdeWorkspaceRegistry,
     project_id: &str,
     path: &str,
     line: u32,
     sealed_input_verified: bool,
 ) -> Option<SourceAnchor> {
     if !sealed_input_verified || !safe_path(path) || line == 0 {
         return None;
     }
     let target = input.root.join(path);
     let meta = fs::symlink_metadata(&target).ok()?;
     if !meta.is_file() || meta.file_type().is_symlink() ||
         meta.len() > MAX_SOURCE_BYTES {
         return None;
     }
     let bytes = fs::read(&target).ok()?;
     if bytes.len() as u64 != meta.len() { return None; }
     let text = std::str::from_utf8(&bytes).ok()?;
     if line as usize > text.lines().count().max(1) {
         return None;
     }
     let digest = format!("{:x}", Sha256::digest(&bytes));
     let current = registry.preview_source_file(project_id,path).ok()?;
     if current.project_id != project_id || current.relative_path != path ||
         current.revision != digest {
         return None;
     }
     Some(SourceAnchor {
         relative_path: path.to_owned(),
         disk_sha256: digest,
         line, character_utf16: 0,
     })
 }

 #[allow(clippy::too_many_arguments)]
 pub fn reported_cases(
     registry: &IdeWorkspaceRegistry,
     input: &PublishedInput,
     capture: &OutputPage,
     project_id: &str,
     job_id: &str,
     recipe_id: &str,
     plan_sha256: &str,
     terminal_receipt_sha256: &str,
     source_head: &str,
     toolchain_sha256: &str,
     native_terminated: bool,
 ) -> TestCaseReport {
     let (lines, observed_log_sha256, mut capture_truncated) = capture_lines(capture);
     if !native_terminated {capture_truncated=true;}
     let sealed_input_verified = native_terminated && verify_input(input).is_ok();
     let mut observed_count=0;
     let mut candidates=Vec::<Candidate>::new();
     let mut candidate_names = BTreeSet::<String>::new();
     let mut pytest_lines = BTreeMap::<String,BTreeSet<u32>>::new();
     for line in &lines {
         if let Some((path,line_number)) = reported_pytest_location(line.trim()) {
             pytest_lines.entry(path).or_default().insert(line_number);
         }
         if !native_terminated {continue;}
         if let Some(candidate) = parse_candidate(recipe_id,line) {
             // Duplicate names are ambiguous across binary test runners and
             // cannot certify a distinct case. Keep one bounded observation.
             let key=format!("{}:{}:{}",candidate.reporter,candidate.name,candidate.status);
             if !candidate_names.insert(key) {continue;}
             observed_count+=1;
             if candidates.len()<MAX_CASES {candidates.push(candidate);}
         }
     }
     let cases_truncated=observed_count>MAX_CASES;
     let mut failed_by_path = BTreeMap::<String,usize>::new();
     for candidate in &candidates {
         if candidate.status=="reported_fail" {
             if let Some(path) = &candidate.path {
                 *failed_by_path.entry(path.to_owned()).or_default()+=1;
             }
         }
     }
     let cases = candidates.into_iter().enumerate().map(|(index,candidate)| {
         let source = if candidate.status=="reported_fail" {
             candidate.path.as_ref().and_then(|path| {
                 let positions = pytest_lines.get(path)?;
                 if positions.len()!=1 ||
                     failed_by_path.get(path)!=Some(&1) {return None;}
                 sealed_source_anchor(input,registry,project_id,path,
                     *positions.iter().next()?,sealed_input_verified)
             })
         } else {None};
         let mut digest=Sha256::new();
         digest.update(job_id.as_bytes());
         digest.update((index as u64).to_le_bytes());
         digest.update(candidate.name.as_bytes());
         TestCaseEvidence {
             case_id:format!("case_{:x}",digest.finalize()),
             name:candidate.name,status:candidate.status.to_owned(),
             reporter:candidate.reporter,source,
             navigation_verified:false,assertion_independently_verified:false,
         }
     }).collect();
     TestCaseReport {
         schema_version:1,
         project_id:project_id.to_owned(),
         job_id:job_id.to_owned(),
         recipe_id:recipe_id.to_owned(),
         plan_sha256:plan_sha256.to_owned(),
         terminal_receipt_sha256:terminal_receipt_sha256.to_owned(),
         source_head:source_head.to_owned(),
         input_snapshot_sha256:input.seal.source_sha256.clone(),
         manifest_sha256:input.seal.manifest_sha256.clone(),
         toolchain_sha256:toolchain_sha256.to_owned(),
         observed_log_sha256,capture_truncated,
         cases_truncated,case_count_observed:observed_count,cases,
         evidence_state:if native_terminated {
             "native_terminal_framework_reported_not_assertion_verified"
         } else {
             "native_not_reconciled_test_evidence_suppressed"
         },
         assertion_independently_verified:false,
         accepted_by_human:false,
         installed_app_accepted:false,
     }
 }

 #[cfg(test)]
 mod tests {
     use super::*;
     #[test]
     fn fixed_reporters_denied_arbitrary_commands_and_malformed_tests() {
         assert!(parse_candidate("rust_tests","test parser::works ... ok").is_some());
         assert!(parse_candidate("js_tests","not ok 2 - fails").is_some());
         assert!(parse_candidate("cpp_cmake_build",
             "1/1 Test #1: sample ........ ***Failed").is_some());
         assert!(parse_candidate("python_pytest",
             "FAILED tests/test_math.py::test_subtract - AssertionError").is_some());
         assert!(parse_candidate("rust_build","test pretend ... ok").is_none());
         assert!(parse_candidate("js_tests","ok bash - rm").is_none());
         assert!(reported_pytest_location("../../secret.py:4: AssertionError").is_none());
         assert!(!safe_path("C:/private.py"));
     }
     #[test]
     fn streams_cannot_be_cross_spliced_into_a_fake_case() {
         use lu_ide_execution_supervisor::live_output::OutputChunk;
         let capture=OutputPage {chunks:vec![
             OutputChunk{sequence:1,stream:"stdout",text:"test fake".into()},
             OutputChunk{sequence:2,stream:"stderr",text:" ... ok\n".into()},
         ],last_sequence:2,truncated:false,live_unverified:true};
         let (lines,_,partial)=capture_lines(&capture);
         assert!(!lines.iter().any(|line|parse_candidate("rust_tests",line).is_some()));
         assert!(partial);
     }
 }
