"""Download and build the third-party reference code used for comparisons.

The downloaded sources are LGPL or GPL licensed. They live only under
``corpus/data/`` (git-ignored) and are never part of this repository.
"""

import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

CORPUS_DIR = Path(__file__).resolve().parents[1]
DATA_DIR = CORPUS_DIR / "data"
REFERENCE_DIR = DATA_DIR / "reference"
BUILD_DIR = DATA_DIR / "build"

CDT_URL = "https://github.com/MarcoAttene/CDT"
CDT_COMMIT = "2a71f943b5ff6130f7f37b20c0d6397b0454ea87"
IP_URL = "https://github.com/MarcoAttene/Indirect_Predicates"
IP_COMMIT = "f8f243d18a6851fe3128b7e09af5b3574259d700"

CDT_DIR = REFERENCE_DIR / "cdt-legacy"
IP_DIR = REFERENCE_DIR / "indirect-predicates"
CDT_BUILD_DIR = BUILD_DIR / "cdt-original"


class ReferenceError(Exception):
    """A reference checkout is missing, wrong or could not be built."""


@dataclass(frozen=True)
class Repo:
    """A repository pinned to one commit and the directory it lives in."""

    url: str
    commit: str
    dest: Path


REPOS = [Repo(CDT_URL, CDT_COMMIT, CDT_DIR), Repo(IP_URL, IP_COMMIT, IP_DIR)]


def _git(cwd, *args):
    result = subprocess.run(
        ["git", *args], cwd=cwd, capture_output=True, text=True
    )
    if result.returncode != 0:
        raise ReferenceError(
            f"git {' '.join(args)} failed in {cwd}: {result.stderr.strip()}"
        )
    return result.stdout.strip()


def _check_existing(repo):
    """Raise unless the checkout is at the pinned commit with no tracked changes.

    Untracked files (such as outputs written by running the examples) are
    ignored; submodules count as tracked content.
    """
    head = _git(repo.dest, "rev-parse", "HEAD")
    if head != repo.commit:
        raise ReferenceError(
            f"{repo.dest} is at {head}, expected {repo.commit}; "
            "remove the directory to fetch it again"
        )
    dirty = _git(repo.dest, "status", "--porcelain", "--untracked-files=no")
    if dirty:
        raise ReferenceError(
            f"{repo.dest} has modified tracked files; "
            "remove the directory to fetch it again"
        )


def _clone(repo):
    repo.dest.parent.mkdir(parents=True, exist_ok=True)
    try:
        _git(repo.dest.parent, "clone", "--quiet", repo.url, str(repo.dest))
        _git(repo.dest, "checkout", "--quiet", "--detach", repo.commit)
        if (repo.dest / ".gitmodules").exists():
            _git(repo.dest, "submodule", "update", "--init", "--recursive", "--quiet")
        _check_existing(repo)
    except ReferenceError:
        shutil.rmtree(repo.dest, ignore_errors=True)
        raise


def fetch(repos=None):
    """Make every repository exist at its pinned commit.

    Existing checkouts are verified first, and nothing is changed if any of
    them is at another commit or has modified tracked files.
    """
    repos = REPOS if repos is None else repos
    for repo in repos:
        if repo.dest.exists():
            _check_existing(repo)
    for repo in repos:
        if not repo.dest.exists():
            print(f"fetching {repo.url} at {repo.commit[:12]}")
            _clone(repo)
        else:
            print(f"{repo.dest} is already at {repo.commit[:12]}")


def _run(command, **kwargs):
    if subprocess.run(command, **kwargs).returncode != 0:
        raise ReferenceError(f"command failed: {' '.join(map(str, command))}")


def build_cdt(source=CDT_DIR, build=CDT_BUILD_DIR):
    """Build the original mesher out of tree and return the `cdt` executable."""
    if not source.exists():
        raise ReferenceError(f"{source} is missing; run fetch-reference first")
    _check_existing(Repo(CDT_URL, CDT_COMMIT, source))
    if shutil.which("cmake") is None:
        raise ReferenceError("cmake was not found in PATH")
    generator = ["-G", "Ninja"] if shutil.which("ninja") else []
    _run(
        ["cmake", "-S", str(source), "-B", str(build), *generator,
         "-DLGPL=ON", "-DCMAKE_BUILD_TYPE=Release"]
    )
    _run(["cmake", "--build", str(build), "--config", "Release"])
    executable = build / "cdt"
    if not executable.is_file():
        raise ReferenceError(f"the build did not produce {executable}")
    return executable
