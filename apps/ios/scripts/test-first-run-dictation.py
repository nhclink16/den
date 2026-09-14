#!/usr/bin/env python3
"""Run the real first-use dictation UI test after resetting one explicit simulator.

Uses already-built native test products. Without --fixture, builds the existing
portable Rust fixture and provisions disposable accounts, then removes only that
fixture on exit. No microphone capture is used by the app's Debug PCM source.
Only the selected simulator is booted if needed. No automatic retries are performed.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--simulator", required=True, help="Exact simulator UUID")
    products_group = parser.add_mutually_exclusive_group(required=True)
    products_group.add_argument("--xctestrun", type=Path, help="Existing native .xctestrun file")
    products_group.add_argument("--test-products", type=Path, help="Existing .xctestproducts bundle directory")
    parser.add_argument("--output", required=True, type=Path, help="New private proof directory; must not exist")
    parser.add_argument("--fixture", type=Path, help="Existing isolated fixture to reuse without stopping it")
    parser.add_argument("--server-binary", type=Path, help="Existing den-server; otherwise build the portable fixture archive")
    parser.add_argument("--all-ui-tests", action="store_true", help="Run all DenUITests, including first-run dictation")
    arguments = parser.parse_args()
    simulator = str(uuid.UUID(arguments.simulator)).upper()
    products = (arguments.test_products or arguments.xctestrun).expanduser().resolve(strict=True)
    if arguments.test_products:
        if not products.is_dir() or products.suffix != ".xctestproducts":
            parser.error("--test-products must identify an existing .xctestproducts bundle directory")
        products_option = "-testProductsPath"
    else:
        if not products.is_file() or products.suffix != ".xctestrun":
            parser.error("--xctestrun must identify an existing .xctestrun file")
        products_option = "-xctestrun"
    if arguments.fixture and arguments.server_binary:
        parser.error("Use --fixture or --server-binary, not both")
    output = arguments.output.absolute()
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    output = output.resolve()
    scripts = Path(__file__).resolve().parents[1] / "ci_scripts"
    fixture = arguments.fixture.resolve(strict=True) if arguments.fixture else output / "fixture.json"
    owned_fixture = False
    status = 1
    command = []
    try:
        if arguments.fixture is None:
            if arguments.server_binary:
                binary = arguments.server_binary.resolve(strict=True)
            else:
                binary = Path(subprocess.check_output([str(scripts / "build-fixture-server.sh")], text=True).strip())
            subprocess.run([sys.executable, str(scripts / "fixture.py"), "start", "--binary", str(binary),
                            "--credentials", str(fixture)], check=True)
            owned_fixture = True
        subprocess.run([sys.executable, str(scripts / "reset-dictation-privacy.py"),
                        "--simulator", simulator, "--fixture", str(fixture)], check=True)
        selection = "DenUITests" if arguments.all_ui_tests else "DenUITests/TextFlowTests/testFirstRunDictationPermissionsAndFirstPCMFrame"
        command = ["/usr/bin/xcodebuild", "test-without-building", products_option, str(products),
                   "-destination", "platform=iOS Simulator,id=" + simulator,
                   "-parallel-testing-enabled", "NO",
                   "-only-testing:" + selection, "-resultBundlePath", str(output / "first-run.xcresult")]
        environment = os.environ.copy()
        environment["TEST_RUNNER_DEN_UI_FIXTURE_PATH"] = str(fixture)
        environment["TEST_RUNNER_DEN_UI_REQUIRE_FIXTURE"] = "1"
        with (output / "xcodebuild.log").open("w") as log:
            status = subprocess.run(command, env=environment, stdout=log, stderr=subprocess.STDOUT).returncode
    finally:
        (output / "run.json").write_text(json.dumps({"simulatorID": simulator,
            "xctestrun": str(products) if arguments.xctestrun else None,
            "testProductsPath": str(products) if arguments.test_products else None,
            "fixtureOwned": owned_fixture, "command": command, "exitCode": status}, indent=2) + "\n")
        if owned_fixture:
            subprocess.run([sys.executable, str(scripts / "fixture.py"), "stop", "--credentials", str(fixture)], check=True)
    print(f"First-run dictation exited {status}; retained proof: {output}")
    return status


if __name__ == "__main__":
    os.umask(0o077)
    sys.exit(main())
