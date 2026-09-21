import argparse
import ctypes
import hashlib
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CORPUS_SHA256 = "0e8b21bc8c28a88a90d891d91020a21aab95a2cfa83f761dd2b1642d98c757ea"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=True, indent=2) + "\n", encoding="utf-8", newline="\n")


def metrics(counts):
    positive, missed = counts["true_positives"], counts["false_negatives"]
    unwanted, negative = counts["false_positives"], counts["true_negatives"]
    def divide(numerator, denominator):
        return numerator / denominator if denominator else 0.0
    return {"precision": divide(positive, positive + unwanted), "recall": divide(positive, positive + missed),
            "f1": divide(2 * positive, 2 * positive + missed + unwanted),
            "accuracy": divide(positive + negative, positive + negative + missed + unwanted)}


def timing_summary(samples):
    values = [value / 1e6 for value in samples]
    quartiles = statistics.quantiles(values, n=4, method="inclusive") if len(values) > 1 else values * 3
    spread = quartiles[2] - quartiles[0]
    return {"count": len(values), "min_ms": min(values), "median_ms": statistics.median(values),
            "mean_ms": statistics.mean(values), "max_ms": max(values),
            "stdev_ms": statistics.stdev(values) if len(values) > 1 else 0.0,
            "q1_ms": quartiles[0], "q3_ms": quartiles[2],
            "outlier_rule": "outside Q1 - 1.5 IQR or Q3 + 1.5 IQR; no samples removed",
            "outlier_samples_1based": [index + 1 for index, value in enumerate(values)
                                       if value < quartiles[0] - 1.5 * spread or value > quartiles[2] + 1.5 * spread]}


def build_workers(arguments, environment):
    environment = dict(environment)
    for key in list(environment):
        if key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS") or key.startswith("CARGO_PROFILE_"):
            del environment[key]
    environment.update({"GOTOOLCHAIN": "go1.27.1", "GOFLAGS": "", "GOAMD64": "v1", "CARGO_BUILD_JOBS": "1"})
    cargo = [arguments.cargo, f"+{arguments.rust_toolchain}"]
    go_command = [arguments.go, "build", "-mod=readonly", "-trimpath", "-pgo=off", "-o", str(arguments.go_binary.resolve()), str(ROOT / "tools/go-reference/benchmark.go")]
    rust_command = [*cargo, "build", "--locked", "--release", "--example", "benchmark", "--target-dir", str(arguments.rust_binary.resolve().parents[2]), "--message-format=json-render-diagnostics"]
    subprocess.run(go_command, cwd=arguments.go_source, env=environment, check=True)
    output = subprocess.check_output(rust_command, cwd=ROOT, env=environment, text=True)
    artifacts = [json.loads(line) for line in output.splitlines()]
    artifact = next(item for item in artifacts if item.get("reason") == "compiler-artifact" and item["target"]["name"] == "benchmark" and item.get("executable"))
    if Path(artifact["executable"]).resolve() != arguments.rust_binary.resolve():
        raise ValueError("Built Rust executable does not match --rust-binary")
    metadata = json.loads(subprocess.check_output([*cargo, "metadata", "--locked", "--format-version", "1"], cwd=ROOT, env=environment))
    features = {item["id"]: item["features"] for item in metadata["resolve"]["nodes"]}
    dependencies, local, local_paths, model = [], {}, {}, {}
    for package in metadata["packages"]:
        dependencies.append({key: package[key] for key in ("name", "version", "source")}
                            | {"features": features.get(package["id"], [])})
        directory = Path(package["manifest_path"]).parent
        if package["source"] is None and directory != ROOT:
            files = sorted(path for path in (directory / "src").rglob("*") if path.is_file())
            files += [directory / "Cargo.toml"]
            files += [path for path in (directory / "Cargo.lock", directory / "build.rs", directory / "README.md") if path.is_file()]
            local[package["name"]] = {str(path.relative_to(directory)): digest(path) for path in files}
            local_paths[package["name"]] = str(directory)
        if package["name"] == "rust-py3langid":
            model = {str(path.relative_to(directory)): digest(path) for path in sorted(directory.rglob("*")) if path.is_file()}
    cargo_path = Path(shutil.which(arguments.cargo) or arguments.cargo)
    rustc = cargo_path.with_name("rustc.exe" if os.name == "nt" else "rustc")
    return {"commands": {"go": go_command, "rust": rust_command},
            "go_build_info": subprocess.check_output([arguments.go, "version", "-m", str(arguments.go_binary)], env=environment, text=True),
            "rustc": subprocess.check_output([str(rustc), f"+{arguments.rust_toolchain}", "-vV"], env=environment, text=True),
            "cargo": subprocess.check_output([*cargo, "--version"], env=environment, text=True).strip(),
            "rust_profile": artifact["profile"], "dependencies": dependencies,
            "local_dependency_source_sha256": local, "local_dependency_paths": local_paths,
            "language_package_files_sha256": model,
            "environment": {key: value for key, value in environment.items()
                            if key in ("GOWORK", "GOMAXPROCS", "CGO_ENABLED", "TZ", "GOFLAGS", "GOTOOLCHAIN", "GOAMD64", "CARGO_BUILD_JOBS")
                            or key.startswith("CARGO_TARGET_")},
            "binary_sha256": {"go": digest(arguments.go_binary), "rust": digest(arguments.rust_binary)}}


class Worker:
    def __init__(self, command, environment):
        self.process = subprocess.Popen(command, cwd=ROOT, env=environment, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, text=True, encoding="utf-8")
        try:
            self.ready = self.read()
        except BaseException:
            self.process.kill()
            self.process.wait()
            raise

    def read(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"Worker terminated: {self.process.poll()}")
        return json.loads(line)

    def run(self, request):
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        return self.read()

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=30)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait()
        finally:
            self.process.stdout.close()


def differences(reference, actual):
    if len(reference) != len(actual):
        raise ValueError("Output lengths differ")
    result = []
    for first, second in zip(reference, actual):
        if first["file"] != second["file"]:
            raise ValueError("Output ordering differs")
        fields = [name for name in first if name not in ("file", "input_tree") and first[name] != second[name]]
        if fields:
            result.append({"file": first["file"], "fields": fields,
                           "text_without_whitespace_equal": "".join(first["text"].split()) == "".join(second["text"].split())})
    return result


def set_affinity(cpu):
    if hasattr(os, "sched_setaffinity"):
        os.sched_setaffinity(0, {cpu})
    elif os.name == "nt":
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(), 1 << cpu):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        raise RuntimeError("CPU affinity is unavailable on this platform")


def selected_content_comparison(reference, actual):
    excluded, different = [], []
    for expected, result in zip(reference, actual, strict=True):
        if expected["file"] != result["file"]:
            raise ValueError("Output ordering differs")
        if expected["error"].startswith(("DOM import unavailable:", "Python reference exception:")):
            excluded.append({"file": expected["file"], "error": expected["error"]})
            continue
        fields = [field for field in ("selected_text", "selected_comments")
                  if "".join(expected[field].split()) != "".join(result[field].split())]
        if bool(expected["error"]) != bool(result["error"]):
            fields.append("acceptance")
        if fields:
            different.append({"file": expected["file"], "fields": fields})
    return {"comparison": "selected DOM text ignoring whitespace, plus acceptance; not serializer or metadata equality",
            "compared": len(reference) - len(excluded), "exclusions": excluded, "differences": different}


def main():
    parser = argparse.ArgumentParser(description="Python non-fallback and Go/Rust native quality/performance comparison")
    parser.add_argument("--corpus", type=Path, default=ROOT.parent / "rust-domdistiller/target/benchmark/corpus.json")
    parser.add_argument("--go-source", type=Path, required=True)
    parser.add_argument("--go-binary", type=Path, required=True)
    parser.add_argument("--rust-binary", type=Path, required=True)
    parser.add_argument("--build", action="store_true", help="Build both workers and record their toolchains and dependency fingerprints")
    parser.add_argument("--go", default="go")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rust-toolchain", default="1.98.1")
    parser.add_argument("--python", type=Path)
    parser.add_argument("--upstream", type=Path)
    parser.add_argument("--git", default="git")
    parser.add_argument("--output", type=Path, default=ROOT / "target/benchmark/results.json")
    parser.add_argument("--samples", type=int, default=0)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--mode", choices=("core", "fallback"), action="append")
    parser.add_argument("--focus", choices=("balanced", "precision", "recall"), default="balanced")
    parser.add_argument("--comments", action="store_true")
    parser.add_argument("--images", action="store_true")
    parser.add_argument("--links", action="store_true")
    parser.add_argument("--input-tree", action="store_true")
    parser.add_argument("--python-same-dom", action="store_true")
    parser.add_argument("--python-preprocess-comments", action="store_true")
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--allow-native-differences", action="store_true")
    arguments = parser.parse_args()
    if arguments.samples < 0 or arguments.warmups < 0:
        parser.error("sample and warmup counts must be nonnegative")
    if arguments.python and not arguments.upstream:
        parser.error("--python requires --upstream")
    if arguments.python_same_dom and not arguments.python:
        parser.error("--python-same-dom requires --python")
    if arguments.python_preprocess_comments and not arguments.python_same_dom:
        parser.error("--python-preprocess-comments requires --python-same-dom")
    corpus = arguments.corpus.resolve()
    if digest(corpus) != CORPUS_SHA256:
        raise ValueError("Corpus checksum differs from the agreed 983-page benchmark")
    pages = json.loads(corpus.read_bytes())
    if (len(pages), sum(len(page["with"]) for page in pages), sum(len(page["without"]) for page in pages)) != (983, 2935, 2948):
        raise ValueError("Corpus annotation counts differ")
    environment = {**os.environ, "GOWORK": "off", "GOMAXPROCS": "1", "CGO_ENABLED": "0", "TZ": "UTC"}
    build_path = arguments.output.parent / "build.json"
    if arguments.build:
        build = build_workers(arguments, environment)
        save(build_path, build)
    elif build_path.is_file():
        build = json.loads(build_path.read_bytes())
        if build["binary_sha256"] != {"go": digest(arguments.go_binary), "rust": digest(arguments.rust_binary)}:
            raise ValueError("Recorded builds do not match the supplied binaries; rerun with --build")
    else:
        build = None
    if arguments.samples and build is None:
        parser.error("timing requires --build or matching build.json beside --output")
    set_affinity(arguments.cpu)
    git = lambda *args: subprocess.check_output([arguments.git, *args])
    source_files = sorted(ROOT.glob("src/**/*.rs")) + sorted(ROOT.glob("examples/*.rs")) + sorted(ROOT.glob("tools/**/*.py"))
    source_files += [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "tools/go-reference/benchmark.go")]
    go_files = sorted(arguments.go_source.glob("**/*.go")) + [arguments.go_source / "go.mod", arguments.go_source / "go.sum"]
    record = {
        "created_utc": datetime.now(timezone.utc).isoformat(), "corpus_sha256": CORPUS_SHA256,
        "corpus_commit": "466fdbee8a504441eb78ed11d71c1da220681cab", "pages": len(pages),
        "positive_snippets": 2935, "negative_snippets": 2948, "platform": platform.platform(),
        "cpu": platform.processor(), "logical_cpu": arguments.cpu, "logical_cpu_count": os.cpu_count(),
        "python_controller": sys.version, "environment": {key: environment[key] for key in ("GOWORK", "GOMAXPROCS", "CGO_ENABLED", "TZ")},
        "go_head": git("-C", str(arguments.go_source), "rev-parse", "HEAD").decode().strip(),
        "go_worktree_patch_sha256": hashlib.sha256(git("-C", str(arguments.go_source), "diff", "HEAD", "--", "*.go", "go.mod", "go.sum")).hexdigest(),
        "go_worktree_source_sha256": {str(path.relative_to(arguments.go_source)): digest(path) for path in go_files},
        "rust_source_sha256": {str(path.relative_to(ROOT)): digest(path) for path in source_files},
        "binary_sha256": {"go": digest(arguments.go_binary), "rust": digest(arguments.rust_binary)},
        "build": build,
        "timing_scope": "pre-parsed DOM extraction including metadata/date/language when configured, internal fallback adapters and output construction, result destruction and case-sensitive snippet scoring; excludes file reads, initial decoding/parsing, IPC, and optional diagnostic output serialization",
        "warmups": arguments.warmups, "samples": arguments.samples, "modes": {},
        "python_input": "Go parsed DOM, preserving comments and text boundaries" if arguments.python_same_dom else "Python load_html",
        "python_comment_preprocessing": arguments.python_preprocess_comments,
    }
    workers = {}
    try:
        workers["go"] = Worker([str(arguments.go_binary.resolve()), str(corpus)], environment)
        workers["rust"] = Worker([str(arguments.rust_binary.resolve()), str(corpus)], environment)
        if arguments.python:
            workers["python"] = Worker([str(arguments.python.resolve()), "-I", "-B", str(ROOT / "tools/python_reference.py"),
                                        "--upstream", str(arguments.upstream), "--go-source", str(arguments.go_source),
                                        "--git", arguments.git, "--corpus", str(corpus)], environment)
        for name, worker in workers.items():
            if worker.ready["ready"] != len(pages):
                raise ValueError(f"Unexpected {name} ready count")
        record["reference"] = {name: worker.ready for name, worker in workers.items()}
        for mode in arguments.mode or ("core", "fallback"):
            request = {"fallback": mode == "fallback", "focus": arguments.focus, "comments": arguments.comments,
                       "images": arguments.images, "links": arguments.links, "formatting": True}
            mode_record = {"options": request, "quality": {}, "warmups": [], "samples": []}
            quality = {}
            for name, worker in workers.items():
                if name == "python" and mode != "core":
                    continue
                diagnostic = {**request, "outputs": True, "input_tree": arguments.input_tree or arguments.python_same_dom}
                if name == "python" and arguments.python_same_dom:
                    diagnostic["input_documents"] = [output["input_tree"] for output in quality["go"]]
                    diagnostic["preprocess_comments"] = arguments.python_preprocess_comments
                response = worker.run(diagnostic)
                quality[name] = response["outputs"]
                save(arguments.output.parent / f"{mode}-{name}.json", response)
                mode_record["quality"][name] = {"counts": response["counts"], "errors": response["errors"], **metrics(response["counts"])}
                print(f"{mode} {name}: {metrics(response['counts'])}; {len(response['errors'])} rejected", flush=True)
            mode_record["go_rust_differences"] = differences(quality["go"], quality["rust"])
            print(f"{mode}: {len(mode_record['go_rust_differences'])}/{len(pages)} Go/Rust results differ", flush=True)
            if "python" in quality:
                for name in ("go", "rust"):
                    mode_record[f"python_{name}_differences"] = differences(quality["python"], quality[name])
                    comparison = selected_content_comparison(quality["python"], quality[name])
                    mode_record[f"python_{name}_selected_content"] = comparison
                    print(f"core Python/{name} selected content: {len(comparison['differences'])}/{comparison['compared']} differ; {len(comparison['exclusions'])} reference exclusions", flush=True)
            record["modes"][mode] = mode_record
            save(arguments.output, record)
            if arguments.samples and mode_record["go_rust_differences"] and not arguments.allow_native_differences:
                raise RuntimeError("Native results differ; timing is deferred until differences are resolved")
            for index in range(arguments.warmups + arguments.samples if arguments.samples else 0):
                order = ["go", "rust"] if index % 2 == 0 else ["rust", "go"]
                sample = {"order": order}
                for name in order:
                    response = workers[name].run(request)
                    expected = mode_record["quality"][name]
                    if response["counts"] != expected["counts"] or response["errors"] != expected["errors"]:
                        raise RuntimeError(f"Non-repeatable extraction: {mode}/{name}")
                    sample[name] = response["elapsed_ns"]
                group = "warmups" if index < arguments.warmups else "samples"
                mode_record[group].append(sample)
                save(arguments.output, record)
                print(f"{mode} {group} {len(mode_record[group])}: Go {sample['go']/1e6:.2f} ms; Rust {sample['rust']/1e6:.2f} ms", flush=True)
            if mode_record["samples"]:
                mode_record["timing"] = {name: timing_summary([sample[name] for sample in mode_record["samples"]]) for name in ("go", "rust")}
                mode_record["median_ms"] = {name: statistics.median(sample[name] for sample in mode_record["samples"]) / 1e6 for name in ("go", "rust")}
                mode_record["speedup"] = mode_record["median_ms"]["go"] / mode_record["median_ms"]["rust"]
            save(arguments.output, record)
    finally:
        for worker in workers.values():
            worker.close()


if __name__ == "__main__":
    main()