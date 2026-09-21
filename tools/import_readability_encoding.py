import argparse
import hashlib
import io
import tarfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "rust-readability-v2-0.6.0"
SHA256 = "711ea12177c6cecbdffff6c679ab984e0f957ad2374a3d44e83d397f41b6ce7f"


def main():
    parser = argparse.ArgumentParser(description="Import the pinned decoder and tables; the encoding host module has local optimizations")
    parser.add_argument("--crate", type=Path)
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    source = arguments.crate
    if source is None:
        matches = list((Path.home() / ".cargo/registry/cache").glob(f"*/{PACKAGE}.crate"))
        if len(matches) != 1:
            raise ValueError("Specify --crate with the downloaded rust-readability-v2 0.6.0 archive")
        source = matches[0]
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Readability archive checksum does not match the published version")
    files = {
        "src/encoding/decoder.rs": "src/encoding/decoder.rs",
        "src/encoding/tables.rs": "src/encoding/tables.rs",
        "LICENSE": "LICENSE-READABILITY-ENCODING",
    }
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for origin, destination in files.items():
            content = archive.extractfile(f"{PACKAGE}/{origin}").read()
            output = ROOT / destination
            if arguments.write:
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(content)
            elif not output.exists() or output.read_bytes() != content:
                raise ValueError(f"Pinned encoding source differs: {output}")
            print(f"{destination}: {hashlib.sha256(content).hexdigest()}")


if __name__ == "__main__":
    main()