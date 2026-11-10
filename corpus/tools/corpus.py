#!/usr/bin/env python3
"""Single entry point for the corpus tooling.

Add a subcommand by writing a function that takes the parsed arguments and
registering it in ``COMMANDS``.
"""

import argparse
import sys

import reference


def fetch_reference(_args):
    reference.fetch()


def build(_args):
    print(f"built {reference.build_cdt()}")


COMMANDS = {
    "fetch-reference": (
        fetch_reference,
        "download the reference repositories at their pinned commits",
    ),
    "build": (build, "build the original reference mesher"),
}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name, (_, help_text) in COMMANDS.items():
        commands.add_parser(name, help=help_text)
    args = parser.parse_args(argv)
    try:
        COMMANDS[args.command][0](args)
    except reference.ReferenceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
