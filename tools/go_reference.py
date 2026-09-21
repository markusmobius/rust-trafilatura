import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[1]
COMMIT = "72dce36bfe95502563533cf68a9050370a3d7081"
MODULE_HASHES = {
    "go.mod": "b96ae594ce46f503380141a76ce3733cf0342f78ff19ac9e1d4582384e5e851e",
    "go.sum": "975f1ec71578a0a9af40cc68a34d2fee0af42b29eefd45bd2a8b6cffcd70f1a5",
}
REQUIRED_VERSIONS = {
    "github.com/markusmobius/go-htmldate": "v1.10.1",
    "github.com/markusmobius/go-dateparser": "v1.4.7",
    "github.com/markusmobius/go-dateutil/v2": "v2.9.1",
    "github.com/markusmobius/go-readabilityV2": "v0.6.0",
    "github.com/forPelevin/gomoji": "v1.3.0",
    "github.com/rivo/uniseg": "v0.4.7",
    "golang.org/x/text": "v0.42.0",
}


def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def modules_from_json(stream):
    decoder = json.JSONDecoder()
    modules = {}
    while stream.strip():
        module, offset = decoder.raw_decode(stream.lstrip())
        stream = stream.lstrip()[offset:]
        if module.get("Replace"):
            raise ValueError(f"Module replacements are forbidden: {module['Path']}")
        modules[module["Path"]] = module.get("Version", COMMIT)
    return modules


def main():
    parser = argparse.ArgumentParser(description="Export or verify fixtures from the exact unreleased Go reference.")
    parser.add_argument("--go-source", type=Path, required=True, help="Git checkout containing the pinned Go commit; local edits are never used")
    parser.add_argument("--go", default="go")
    parser.add_argument("--git", default="git")
    parser.add_argument("--write", action="store_true", help="Replace generated fixtures and Unicode tables; default is a byte-for-byte check")
    parser.add_argument("--worktree", action="store_true", help="Export labelled current-Go pipeline comparisons without changing the frozen oracle")
    arguments = parser.parse_args()
    environment = dict(os.environ, GOWORK="off", CGO_ENABLED="0", GOTOOLCHAIN="go1.27.1", TZ="UTC")
    with tempfile.TemporaryDirectory(prefix="rust-trafilatura-reference-") as directory:
        temporary = Path(directory)
        source = temporary / "source"
        source.mkdir()
        archive = temporary / "source.tar"
        with archive.open("wb") as output:
            run([arguments.git, "-C", str(arguments.go_source), "-c", "core.autocrlf=false", "archive", "--format=tar", COMMIT], stdout=output)
        with tarfile.open(archive) as incoming:
            incoming.extractall(source, filter="data")
        if arguments.worktree:
            source = arguments.go_source.resolve()
        for name, expected in MODULE_HASHES.items():
            actual = hashlib.sha256((source / name).read_bytes()).hexdigest()
            if actual != expected:
                raise ValueError(f"Pinned {name} checksum mismatch: {actual}")
        run([arguments.go, "mod", "verify"], cwd=source, env=environment)
        result = run([arguments.go, "list", "-mod=readonly", "-m", "-json", "all"], cwd=source, env=environment, capture_output=True, text=True)
        modules = modules_from_json(result.stdout)
        for name, version in REQUIRED_VERSIONS.items():
            if modules.get(name) != version:
                raise ValueError(f"Unexpected reference dependency: {name} {modules.get(name)}")
        graph = temporary / "modules.json"
        graph.write_text(json.dumps(modules, sort_keys=True), encoding="utf-8")
        overlay = temporary / "overlay.json"
        cases_directory = Path(run([arguments.go, "list", "-mod=readonly", "-f", "{{.Dir}}", "golang.org/x/text/cases"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
        cases_source = temporary / "x-text"
        shutil.copytree(cases_directory.parent, cases_source, ignore=shutil.ignore_patterns("*_test.go"))
        graphemes_directory = Path(run([arguments.go, "list", "-mod=readonly", "-f", "{{.Dir}}", "github.com/rivo/uniseg"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
        graphemes_source = temporary / "uniseg"
        shutil.copytree(graphemes_directory, graphemes_source, ignore=shutil.ignore_patterns("*_test.go"))
        emoji_directory = Path(run([arguments.go, "list", "-mod=readonly", "-f", "{{.Dir}}", "github.com/forPelevin/gomoji"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
        css_directory = Path(run([arguments.go, "list", "-mod=readonly", "-f", "{{.Dir}}", "github.com/andybalholm/cascadia"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
        overlay.write_text(json.dumps({"Replace": {
            str(source / "z_rust_reference_test.go"): str(ROOT / "tools/go-reference/export_test.go"),
            str(cases_source / "cases/z_rust_reference_test.go"): str(ROOT / "tools/go-cases/export_test.go"),
            str(graphemes_source / "z_rust_reference_test.go"): str(ROOT / "tools/go-graphemes/export_test.go"),
        }}), encoding="utf-8")
        fixture = temporary / "go-reference.json"
        tables = temporary / "go_unicode.rs"
        case_tables = temporary / "go_cases.rs"
        case_fixture = temporary / "go-cases.json"
        grapheme_tables = temporary / "go_graphemes.rs"
        environment.update(RUST_REFERENCE_OUTPUT=str(fixture), RUST_REFERENCE_UNICODE=str(tables), RUST_REFERENCE_MODULES=str(graph), RUST_REFERENCE_CASES=str(case_tables), RUST_REFERENCE_CASE_FIXTURE=str(case_fixture), RUST_REFERENCE_GRAPHEMES=str(grapheme_tables), RUST_REFERENCE_EMOJI_SOURCE=str(emoji_directory / "data.go"))
        environment["RUST_REFERENCE_CSS_SOURCE"] = str(css_directory)
        run([arguments.go, "test", "-mod=readonly", "-count=1", "-overlay", str(overlay), "-run", "^TestRustExportCaseTables$", "-v", "./cases"], cwd=cases_source, env=environment)
        run([arguments.go, "test", "-mod=readonly", "-count=1", "-overlay", str(overlay), "-run", "^TestRustExportGraphemeTables$", "-v", "."], cwd=graphemes_source, env=environment)
        run([arguments.go, "test", "-mod=readonly", "-count=1", "-overlay", str(overlay), "-run", "^TestRustExportReference$", "-v", "."], cwd=source, env=environment)
        if arguments.worktree:
            reference = json.loads(fixture.read_bytes())
            selected = {name: reference[name] for name in ("handlers", "content", "sequences", "extraction")}
            files = sorted(source.glob("**/*.go")) + [source / "go.mod", source / "go.sum"]
            selected["identity"] = {"kind": "current-go-worktree-cross-port", "head": run([arguments.git, "-C", str(source), "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip(),
                                    "source_sha256": {path.relative_to(source).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}}
            encoded = (json.dumps(selected, ensure_ascii=True, indent=2, sort_keys=True) + "\n").encode()
            destination = ROOT / "testdata/go-worktree.json"
            if arguments.write:
                destination.write_bytes(encoded)
            elif not destination.exists() or destination.read_bytes() != encoded:
                raise ValueError("Current Go worktree reference differs")
            print(f"{destination.relative_to(ROOT)}: sha256={hashlib.sha256(encoded).hexdigest()}; explicitly current worktree, not frozen Go")
            return
        with tables.open("ab") as output:
            output.write(case_tables.read_bytes())
            output.write(grapheme_tables.read_bytes())
        go_root = Path(run([arguments.go, "env", "GOROOT"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
        generated = {
            ROOT / "testdata/go-reference.json": fixture,
            ROOT / "src/go_unicode.rs": tables,
            ROOT / "LICENSE": source / "LICENSE",
            ROOT / "LICENSE-GO.txt": go_root / "LICENSE",
        }
        for destination, original in generated.items():
            if arguments.write:
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original, destination)
            elif not destination.is_file() or original.read_bytes() != destination.read_bytes():
                raise ValueError(f"Generated reference differs: {destination.relative_to(ROOT)}")
            digest = hashlib.sha256(original.read_bytes()).hexdigest()
            print(f"{destination.relative_to(ROOT)}: sha256={digest}")
        print(f"Pinned Go {COMMIT}: {'exported' if arguments.write else 'byte-identical'}; {len(modules)} selected modules, no replacements")


if __name__ == "__main__":
    main()