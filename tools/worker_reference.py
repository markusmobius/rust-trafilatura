import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_reference(arguments, binary):
    source = arguments.application.resolve()
    environment = {**os.environ, "GOWORK": "off", "GOTOOLCHAIN": "go1.27.1", "CGO_ENABLED": "0", "TZ": "UTC"}
    with tempfile.TemporaryDirectory(prefix="trafilatura-worker-reference-") as temporary:
        project = Path(temporary)
        originals = []
        for name in ("go.mod", "go.sum"):
            shutil.copy2(source / name, project / name)
            originals.append(source / name)
        for relative in ("cmd/goHTML", "lib/eventloop", "lib/distiller", "lib/date", "lib/meta"):
            destination = project / relative
            destination.mkdir(parents=True)
            for path in (source / relative).glob("*.go"):
                if not path.name.endswith("_test.go"):
                    shutil.copy2(path, destination / path.name)
                    originals.append(path)
        subprocess.run([arguments.go, "mod", "edit", f"-replace=github.com/markusmobius/go-trafilatura/v2={arguments.go_source.resolve()}"],
                       cwd=project, env=environment, check=True)
        subprocess.run([arguments.go, "build", "-mod=mod", "-trimpath", "-o", str(binary), "./cmd/goHTML"],
                       cwd=project, env=environment, check=True)
        return {
            "application_source_sha256": {str(path.relative_to(source)): digest(path) for path in originals},
            "go_source_sha256": {str(path.relative_to(arguments.go_source)): digest(path) for path in sorted(arguments.go_source.rglob("*.go"))},
            "isolated_module": (project / "go.mod").read_text(),
            "isolated_sum_sha256": digest(project / "go.sum"),
            "build_info": subprocess.check_output([arguments.go, "version", "-m", str(binary)], env=environment, text=True),
            "scope": "Unmodified application worker source with current Go-Trafilatura substituted only in a temporary module",
        }


def commands(directory):
    paragraph = "A detailed article describes the experiment, its evidence and the results. " * 8
    html = '<html><head><title>Worker &amp;amp; title</title><meta property="article:published_time" content="2020-01-02"></head><body><article><h1>Worker heading</h1><p>' + paragraph + '<a href="/news?a=1&amp;b=2">linked words</a><strong>Bold</strong></p><p>Second paragraph &amp;amp; final words.</p><figure><img src="/image.jpg" alt="Image"></figure></article></body></html>'
    task = {"HTML": html, "URL": "https://example.org/article", "RunTrafilatura": True, "Verbose": True}
    plain = directory / "input with spaces.html"
    plain.write_text(html, encoding="utf-8")
    masked = directory / "input.xor"
    masked.write_bytes(b"||XOR||" + bytes(byte ^ 255 for byte in html.encode()))
    encode = lambda value: json.dumps(value, ensure_ascii=True)
    result = ["{}", "null", "[]", "not JSON", '{"RunTrafilatura":true,', encode(task)]
    result += [encode({**task, "Verbose": False}), encode({key.lower(): value for key, value in task.items()}),
               encode({**task, "HTMLPath": str(plain), "HTML": "wrong inline HTML"}),
               encode({**task, "HTMLPath": str(masked), "HTML": "wrong inline HTML"}),
               encode({**task, "HTMLPath": str(directory / "missing.html")}),
               encode({**task, "URL": "invalid URL"}), encode({**task, "RunTrafilatura": False}),
               encode({**task, "HTML": "\ufeff" + html}), encode({**task, "Verbose": "wrong type"}),
               encode({**task, "HTMLPath": 17}), encode({**task, "HTMLPath": None}),
               encode(task)[:-1] + ',"html":"<p>Last duplicate field wins.</p>"}',
               encode(task)[:-1] + ',"html":null,"HTML":false}',
               encode(task)[:-1] + ',"runtrafilatura":false,"RunTrafilatura":true}',
               encode({**task, "HTML": "<p>&lt;&gt;&amp; \u2028 \u2029 caf\u00e9</p>"})]
    result += [encode(task)[:-1] + ',"unknown":1e10000}',
               encode(task)[:-1] + ',"unknown":' + '[' * 256 + '0' + ']' * 256 + '}',
               encode({**task, "HTML": html.replace("Worker heading", "Worker \ud800 heading")}),
               encode(task)[:-1] + ',"URL":1e10000}']
    return result


def file_results(binary, cases, directory):
    destination = directory / binary.stem
    destination.mkdir()
    paths = [destination / f"result-{index}.xor" for index in range(len(cases))]
    lines = [f"{command}\t  {path}  " for command, path in zip(cases, paths)]
    lines += ["{}\t" + str(destination / "missing" / "output"), "invalid"]
    process = subprocess.run([str(binary)], input="\n".join(lines) + "\n", text=True, encoding="utf-8",
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True)
    expected = ["ready"] + ["ok"] * len(cases) + ["error"]
    if process.stdout.splitlines() != expected:
        raise AssertionError(f"{binary.name} file protocol: {process.stdout!r} {process.stderr!r}")
    return [json.loads(bytes(byte ^ 255 for byte in path.read_bytes())) for path in paths]


def exact_read(connection, count):
    result = bytearray()
    while len(result) < count:
        data = connection.recv(count - len(result))
        if not data:
            raise EOFError("Worker disconnected before completing its frame")
        result.extend(data)
    return bytes(result)


def frame(message):
    content = json.dumps(message, ensure_ascii=True).encode()
    return struct.pack("!II", len(content), 1024) + content


def receive(connection):
    length, chunk = struct.unpack("!II", exact_read(connection, 8))
    if chunk != 1024 * 1024:
        raise AssertionError(f"Unexpected response chunk size: {chunk}")
    message = json.loads(exact_read(connection, length))
    if message.get("MType") == 1:
        message["Content"] = json.loads(message["Content"])
    return message


def tcp_results(binary, cases):
    with socket.socket() as server:
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        server.settimeout(30)
        process = subprocess.Popen([str(binary), str(server.getsockname()[1]), str(os.getpid()), "readiness"],
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            connection, _ = server.accept()
            with connection:
                connection.settimeout(60)
                ready = receive(connection)
                if ready != {"MType": 0, "GUID": "readiness", "Content": ""}:
                    raise AssertionError(ready)
                first = frame({"GUID": "first", "Command": cases[5]})
                for start in range(0, len(first), 3):
                    connection.sendall(first[start:start + 3])
                results = [receive(connection)]
                messages = [{"GUID": "second", "Command": cases[0]},
                            {"guid": "third", "command": cases[5]},
                            {}, {"GUID": "fourth"}, {"Command": cases[1]}]
                connection.sendall(b"".join(frame(message) for message in messages))
                results.extend(receive(connection) for _ in messages)
            process.communicate(timeout=30)
            if process.returncode:
                raise AssertionError(f"{binary.name} TCP exit {process.returncode}")
            return results
        finally:
            if process.poll() is None:
                process.kill()
                process.communicate()


def main():
    parser = argparse.ArgumentParser(description="Compare the native Trafilatura-only worker with actual Go application protocols")
    parser.add_argument("--application", type=Path, default=ROOT.parent / "GoLang/src")
    parser.add_argument("--go-source", type=Path, required=True)
    parser.add_argument("--go", default="go")
    parser.add_argument("--rust-binary", type=Path, required=True)
    parser.add_argument("--reuse-go", action="store_true")
    arguments = parser.parse_args()
    binary = ROOT / "target" / ("goHTML-reference.exe" if os.name == "nt" else "goHTML-reference")
    report_path = ROOT / "target/worker-reference.json"
    report = json.loads(report_path.read_bytes()) if arguments.reuse_go else build_reference(arguments, binary)
    report.update({"go_binary_sha256": digest(binary), "rust_binary_sha256": digest(arguments.rust_binary), "passed": False})
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="trafilatura-worker-check-") as temporary:
        directory = Path(temporary)
        cases = commands(directory)
        expected = file_results(binary, cases, directory)
        actual = file_results(arguments.rust_binary, cases, directory)
        for index, (first, second) in enumerate(zip(expected, actual)):
            if first != second:
                fields = [key for key in first if first[key] != second[key]]
                raise AssertionError(f"File request {index} differs in {fields}:\nGo: {json.dumps(first, ensure_ascii=True)}\nRust: {json.dumps(second, ensure_ascii=True)}")
        report["file_requests"] = len(cases)
        first, second = tcp_results(binary, cases), tcp_results(arguments.rust_binary, cases)
        if first != second:
            for index, (expected, actual) in enumerate(zip(first, second)):
                if expected != actual:
                    raise AssertionError(f"TCP response {index} differs:\nGo: {json.dumps(expected, ensure_ascii=True)}\nRust: {json.dumps(actual, ensure_ascii=True)}")
        report["tcp_requests"] = len(first)
    report["passed"] = True
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Worker parity passed: {report['file_requests']} persistent file requests, output-path failure, {report['tcp_requests']} partial/back-to-back TCP requests, clean disconnect")


if __name__ == "__main__":
    main()