#!/usr/bin/env python3
"""DEV-04.5D2: independent, CI-only LLVM symbol-line oracle.

Never executes a target or launches LLDB. Input is a *claim*, not an OS
kernel custody witness; even a true semantic mapping cannot authorize a
breakpoint/step/evaluate operation or establish a genuine stopped process.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePath
import re
import stat
import struct
import subprocess
import sys

SCHEMA = "lu.ide.dev045d2.independent-source-line.v1"
MAX_JSON = 8192
HEX64 = re.compile(r"^[a-f0-9]{64}$")
PC = re.compile(r"^0x[0-9a-fA-F]{1,16}$")
FILES = {"source": 2 * 1024 * 1024, "binary": 128 * 1024 * 1024,
         "pdb": 128 * 1024 * 1024}
ALLOWED = {"schema", "platform", "binary", "binarySha256", "source",
           "sourceSha256", "sourceLine", "dapSourcePath",
           "dapInstructionPointer", "runtimeSlide", "targetBindingSha256",
           "pdb", "pdbSha256"}


class Denied(Exception):
    pass


def require(test, code):
    if not test:
        raise Denied(code)


def safe_file(root, relative, expected_digest, ceiling):
    require(isinstance(relative, str) and relative
            and len(relative) <= 256 and not relative.startswith(("/", "\\"))
            and "\\" not in relative and not any(ord(c) < 32 for c in relative),
            "INVALID_RELATIVE_FILE")
    segments = relative.split("/")
    require(all(seg not in ("", ".", "..") for seg in segments),
            "INVALID_RELATIVE_FILE")
    require(isinstance(expected_digest, str) and HEX64.fullmatch(expected_digest),
            "INVALID_FILE_DIGEST")
    current = root
    require(current.is_absolute() and current.is_dir()
            and not current.is_symlink(), "UNTRUSTED_REVIEW_ROOT")
    for seg in segments[:-1]:
        current = current / seg
        require(current.is_dir() and not current.is_symlink(),
                "LINKED_SOURCE_PARENT")
    file = current / segments[-1]
    st = file.lstat()
    require(stat.S_ISREG(st.st_mode) and st.st_nlink == 1
            and 0 < st.st_size <= ceiling, "UNTRUSTED_SOURCE_BYTES")
    # CI fixture files are immutable during review. A production native
    # implementation must use capability/no-follow handles and OS custody.
    contents = file.read_bytes()
    after = file.lstat()
    require((st.st_ino, st.st_dev, st.st_size, st.st_mtime_ns) ==
            (after.st_ino, after.st_dev, after.st_size, after.st_mtime_ns),
            "SOURCE_CHANGED_DURING_REVIEW")
    require(hashlib.sha256(contents).hexdigest() == expected_digest,
            "INDEPENDENT_DIGEST_MISMATCH")
    return file, contents


def pe_pdb_guid_age_matches(executable, pdb, expected_pdb_name):
    """Independent MSF7 PDB Info / PE RSDS GUID-age cross-check.

    Never accept a PDB merely because an unrelated PDB file exists beside
    a PE: the RSDS record must match the PDB's actual Info stream identity.
    This parser is bounded and rejects unknown/truncated/colliding records.
    """
    require(pdb.startswith(b"Microsoft C/C++ MSF 7.00") and len(pdb) >= 56,
            "PDB_STRUCTURE_DENIED")
    require(len(executable) >= 64 and executable[:2] == b"MZ",
            "PE_STRUCTURE_DENIED")
    image_offset = struct.unpack_from("<I", executable, 0x3c)[0]
    require(image_offset < len(executable) - 24
            and executable[image_offset:image_offset + 4] == b"PE\x00\x00",
            "PE_STRUCTURE_DENIED")
    records = []
    for found in re.finditer(b"RSDS", executable):
        at = found.start()
        if at + 24 >= len(executable):
            continue
        end = executable.find(b"\x00", at + 24, min(len(executable), at + 280))
        if end < 0:
            continue
        try:
            original_path = executable[at + 24:end].decode("utf-8")
        except UnicodeError:
            continue
        if original_path.replace("\\", "/").split("/")[-1].casefold() != expected_pdb_name.casefold():
            continue
        age = struct.unpack_from("<I", executable, at + 20)[0]
        records.append((executable[at + 4:at + 20], age))
    require(len(records) == 1 and records[0][1] > 0,
            "PE_RSDS_IDENTITY_INVALID")
    block_size, _, number_blocks, directory_size, _, block_map = struct.unpack_from(
        "<6I", pdb, 32)
    require(512 <= block_size <= 65536 and block_size & (block_size - 1) == 0
            and 0 < number_blocks <= len(pdb) // block_size
            and 8 <= directory_size <= 2 * 1024 * 1024
            and block_map < number_blocks, "PDB_STREAM_DIRECTORY_DENIED")
    blocks_needed = (directory_size + block_size - 1) // block_size
    require(block_map * block_size + blocks_needed * 4 <= len(pdb),
            "PDB_BLOCK_MAP_TRUNCATED")
    directory_blocks = struct.unpack_from("<" + "I" * blocks_needed,
                                           pdb, block_map * block_size)
    require(all(b < number_blocks for b in directory_blocks),
            "PDB_BLOCK_MAP_INVALID")
    directory = b"".join(pdb[b * block_size:(b + 1) * block_size]
                         for b in directory_blocks)[:directory_size]
    nstreams = struct.unpack_from("<I", directory, 0)[0]
    require(2 <= nstreams <= 10000 and 4 + nstreams * 4 <= len(directory),
            "PDB_STREAM_COUNT_INVALID")
    sizes = struct.unpack_from("<" + "I" * nstreams, directory, 4)
    offset = 4 + nstreams * 4
    info = None
    for stream, size in enumerate(sizes):
        if size == 0xffffffff:
            continue
        require(size <= len(pdb), "PDB_STREAM_SIZE_INVALID")
        count = (size + block_size - 1) // block_size
        require(offset + count * 4 <= len(directory),
                "PDB_STREAM_BLOCK_LIST_INVALID")
        locations = struct.unpack_from("<" + "I" * count, directory, offset) if count else ()
        offset += count * 4
        require(all(b < number_blocks for b in locations),
                "PDB_STREAM_OUTSIDE_FILE")
        if stream == 1:
            info = b"".join(pdb[b * block_size:(b + 1) * block_size]
                            for b in locations)[:size]
    require(info is not None and len(info) >= 28, "PDB_INFO_STREAM_MISSING")
    pdb_age = struct.unpack_from("<I", info, 8)[0]
    pdb_guid = info[12:28]
    require(records[0] == (pdb_guid, pdb_age),
            "PE_PDB_GUID_AGE_MISMATCH")


def verify_claim(root, claim, symbolizer, symbolizer_sha):
    require(isinstance(claim, dict) and set(claim) == ALLOWED,
            "CLAIM_SCHEMA_INVALID")
    require(claim["schema"] == SCHEMA, "CLAIM_VERSION_DENIED")
    platform = claim["platform"]
    require(platform in ("linux-elf64", "windows-pe-pdb"), "PLATFORM_DENIED")
    require(isinstance(claim["targetBindingSha256"], str)
            and HEX64.fullmatch(claim["targetBindingSha256"]),
            "BUILD_TARGET_BINDING_MISSING")
    source, source_bytes = safe_file(root, claim["source"],
                                      claim["sourceSha256"], FILES["source"])
    binary, binary_bytes = safe_file(root, claim["binary"],
                                      claim["binarySha256"], FILES["binary"])
    require(binary_bytes[:4] == b"\x7fELF" if platform == "linux-elf64"
            else binary_bytes[:2] == b"MZ", "EXECUTABLE_FORMAT_DENIED")
    if platform == "windows-pe-pdb":
        require(isinstance(claim["pdb"], str)
                and claim["pdb"].lower().endswith(".pdb"),
                "PDB_MISSING")
        pdb, contents = safe_file(root, claim["pdb"],
                                  claim["pdbSha256"], FILES["pdb"])
        require(contents.startswith(b"Microsoft C/C++ MSF 7.00")
                and b"RSDS" in binary_bytes, "PDB_STRUCTURE_DENIED")
        pe_pdb_guid_age_matches(binary_bytes, contents, pdb.name)
    else:
        require(claim["pdb"] is None and claim["pdbSha256"] is None,
                "ELF_UNEXPECTED_PDB")
    require(isinstance(claim["sourceLine"], int)
            and not isinstance(claim["sourceLine"], bool)
            and 1 <= claim["sourceLine"] <= 1000000, "LINE_INVALID")
    try:
        decoded = source_bytes.decode("utf-8")
    except UnicodeError as error:
        raise Denied("SOURCE_NOT_UTF8") from error
    require(claim["sourceLine"] <= len(decoded.splitlines()),
            "LINE_OUTSIDE_PINNED_SOURCE")
    require(claim["dapSourcePath"] == str(source),
            "ADAPTER_SOURCE_NOT_PINNED_FILE")
    address = claim["dapInstructionPointer"]
    slide = claim["runtimeSlide"]
    require(isinstance(address, str) and PC.fullmatch(address),
            "STOP_PC_INVALID")
    require(isinstance(slide, str) and PC.fullmatch(slide),
            "RUNTIME_SLIDE_INVALID")
    pc, bias = int(address, 16), int(slide, 16)
    require(pc > 0 and bias <= pc and pc - bias > 0,
            "STOP_PC_OUTSIDE_RELOCATED_IMAGE")
    tool = Path(symbolizer)
    require(tool.is_absolute() and tool.is_file() and not tool.is_symlink(),
            "UNTRUSTED_ORACLE_TOOL")
    require(HEX64.fullmatch(symbolizer_sha or ""), "ORACLE_PIN_ABSENT")
    tool_bytes = tool.read_bytes()
    require(hashlib.sha256(tool_bytes).hexdigest() == symbolizer_sha,
            "ORACLE_BINARY_MISMATCH")
    command = [str(tool), "--obj=" + str(binary),
               "--output-style=JSON", "--functions=none", "--no-inlines",
               "--adjust-vma=" + slide, address]
    # No shell, no program launch/attach and no path derived from adapter
    # output. This independent native CI tool only parses a prebuilt image.
    process = subprocess.run(command, capture_output=True, timeout=15,
                             check=False)
    require(process.returncode == 0 and len(process.stdout) <= 8192
            and len(process.stderr) <= 8192, "SYMBOLIZER_FAILED")
    try:
        mapped = json.loads(process.stdout)
    except (UnicodeError, ValueError) as error:
        raise Denied("MALFORMED_SYMBOL_OUTPUT") from error
    require(isinstance(mapped, list) and len(mapped) == 1
            and mapped[0].get("Address", "").lower() == address.lower(),
            "SYMBOL_ADDRESS_NOT_MATCHED")
    lines = mapped[0].get("Symbol")
    require(isinstance(lines, list) and len(lines) == 1,
            "SYMBOL_LINE_AMBIGUOUS")
    line = lines[0]
    require(os.path.normcase(os.path.normpath(line.get("FileName", "")))
            == os.path.normcase(os.path.normpath(str(source))),
            "SYMBOL_FILE_MISMATCH")
    require(line.get("Line") == claim["sourceLine"]
            and isinstance(line.get("Line"), int)
            and line.get("Line") > 0, "SYMBOL_LINE_MISMATCH")
    return {
        "schema": "lu.ide.dev045d2.line-oracle-result.v1",
        "platform": platform,
        "independentBinaryAndSourceHashMatch": True,
        "independentSymbolLineAddressMatch": True,
        "exactSourceLine": claim["sourceLine"],
        "sourceSha256": claim["sourceSha256"],
        "binarySha256": claim["binarySha256"],
        "targetBindingSha256": claim["targetBindingSha256"],
        # This validator CANNOT certify the origin of the OS process,
        # image slide, user authorization or native worker custody.
        "stoppedPcKernelAttested": False,
        "runtimeSlideKernelAttested": False,
        "breakpointsSteppingVariablesEnabled": False,
        "installedAppAccepted": False,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", required=True)
    parser.add_argument("--claim", required=True)
    parser.add_argument("--symbolizer", required=True)
    parser.add_argument("--symbolizer-sha256", required=True)
    args = parser.parse_args()
    try:
        root = Path(args.root).absolute()
        claim_file = Path(args.claim)
        require(claim_file.is_file() and claim_file.stat().st_size <= MAX_JSON,
                "CLAIM_FILE_DENIED")
        claim = json.loads(claim_file.read_text(encoding="utf-8"))
        output = verify_claim(root, claim, args.symbolizer,
                              args.symbolizer_sha256)
        print(json.dumps(output, sort_keys=True))
    except (Denied, OSError, ValueError, subprocess.TimeoutExpired) as error:
        # Never print user source, debugger text, paths or raw process output.
        code = str(error) if isinstance(error, Denied) else "ORACLE_IO_DENIED"
        print("DEV045D2_LINE_ORACLE_DENIED:" + code, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
