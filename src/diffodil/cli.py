import argparse
import os
import subprocess


def main():
    parser = argparse.ArgumentParser(description="Git diffs in your browser")

    parser.add_argument(
        "root",
        metavar="PATH",
        help="only git repos below the root will be considered",
    )

    parser.add_argument(
        "--port",
        "-p",
        type=int,
        default=8765,
        help="the port on which the server will listen",
    )

    parser.add_argument(
        "--verbose",
        "-v",
        action="count",
        default=0,
        help="increase verbosity (can be used multiple times)",
    )

    parser.add_argument("--dev", action="store_true", help="run server in dev mode")

    args = parser.parse_args()

    env = os.environ

    env.update({"DIFFODIL_VERBOSITY": str(args.verbose), "DIFFODIL_ROOT": args.root})

    cmd = ["uvicorn"]

    cmd.extend(["--host", "0.0.0.0", "--port", str(args.port), "--log-level", "info"])

    if args.dev:
        cmd.append("--reload")

    cmd.append("diffodil.server:app")

    subprocess.run(cmd, env=env)
