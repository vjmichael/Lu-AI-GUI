"""Validate proposal-only Lu DLC API Protobuf descriptor without shipping it.

No runtime registrations, sockets, installer changes or third-party downloads
beyond CI's pinned Protobuf compiler. This is LU-DLC-API-00 schema evidence.
"""
from __future__ import annotations
import sys
from pathlib import Path
from google.protobuf import descriptor_pb2

REQUIRED={
    "HostEnvelope","HelloRequest","HelloReply","CatalogRequest","CatalogReply",
    "ModelStatusRequest","ModelStatusReply","ModelAdmissionRequest",
    "ModelAdmissionReply","ModelSessionAcquire","ModelSessionLease",
    "InferenceStart","InferenceEvent","InferenceCancel",
    "InferenceRecoveryRequest","ModelLeaseRelease","ResourceRequest",
    "ResourceReply","MemoryQueryRequest","MemoryQueryReply",
    "ArtifactResolveRequest","ArtifactResolveReply","CapabilityInvocation",
    "CapabilityResult","HostError",
}
HOST_METHODS={
    "model_status_request","model_admission_request","model_session_acquire",
    "inference_start","inference_cancel","inference_recovery_request",
    "model_lease_release","capability_invocation",
}
BANNED_FIELD_WORDS={"api_key","private_engine_port","executable_path",
                    "kill_process","unload_model"}


def check(path):
    descriptor=descriptor_pb2.FileDescriptorSet()
    descriptor.ParseFromString(Path(path).read_bytes())
    if len(descriptor.file)!=1:
        raise RuntimeError("DLC ABI fixture must compile exactly one proto")
    source=descriptor.file[0]
    if source.package!="lu.dlc.v1" or source.syntax!="proto3":
        raise RuntimeError("Lu DLC protocol package/syntax drift")
    names={m.name:m for m in source.message_type}
    missing=REQUIRED-names.keys()
    if missing:
        raise RuntimeError("Required first-party DLC services missing: "+str(sorted(missing)))
    envelope=names["HostEnvelope"]
    if len(envelope.oneof_decl)!=1 or envelope.oneof_decl[0].name!="payload":
        raise RuntimeError("HostEnvelope must use single strict payload union")
    members={f.name for f in envelope.field if f.HasField("oneof_index")}
    if not HOST_METHODS.issubset(members):
        raise RuntimeError("Missing host inference/CE message methods")
    for msg in source.message_type:
        for field in msg.field:
            if field.name in BANNED_FIELD_WORDS:
                raise RuntimeError("Forbidden raw engine control field in DLC ABI")
    lease_fields={f.name for f in names["ModelLeaseRelease"].field}
    if lease_fields!={"lease_ref"}:
        raise RuntimeError("A DLC can release only its own lease, not a global model")
    start_fields={f.name for f in names["InferenceStart"].field}
    if not {"session_ref","generation_ref","idempotency_ref","prompt",
            "history","images","approved_project_context_ref"}.issubset(start_fields):
        raise RuntimeError("Inference request lacks typed project/session history")
    event_fields={f.name for f in names["InferenceEvent"].field}
    if not {"session_ref","generation_ref","sequence","terminal","chunk"}.issubset(event_fields):
        raise RuntimeError("Inference stream missing ordered terminal evidence")
    cap_fields={f.name for f in names["CapabilityInvocation"].field}
    if not {"capability_id","call_ref","input_type_url","typed_input"}.issubset(cap_fields):
        raise RuntimeError("Cognitum CE typed operation missing from common API")
    print("LU_DLC_API_RFC_SCHEMA_PASS",
          "package="+source.package,"messages="+str(len(names)),
          "host_methods="+str(len(members)),
          "runtime_implemented=false")
    return True


if __name__=="__main__":
    if len(sys.argv)!=2:
        raise SystemExit("Usage: check_dlc_host_contract.py DESCRIPTOR_SET")
    check(sys.argv[1])
